//! Auto-paste on Linux (VRT-59, decision-11): a RemoteDesktop portal session with keyboard
//! access and the Clipboard portal. The dictated text is typed into the focused window through
//! the portal's EIS connection (a libei client: `reis`), with the desktop's keyboard layout
//! (`keys.rs`), once the user has let go of the shortcut's modifiers. Characters the layout
//! can't type go on the clipboard through the portal and are pasted with Ctrl+V (or
//! Ctrl+Shift+V); afterwards the previous clipboard text is put back. GNOME asks once; the
//! restore token it returns after every start lets the next start skip the dialog.

use std::io::{Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileExt;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ashpd::desktop::clipboard::{Clipboard, SetSelectionOptions};
use ashpd::desktop::remote_desktop::{
    ConnectToEISOptions, DeviceType, RemoteDesktop, SelectDevicesOptions, StartOptions,
};
use ashpd::desktop::{CreateSessionOptions, PersistMode, Session};
use futures_util::StreamExt;
use reis::ei;
use reis::event::{DeviceCapability, EiEvent};
use tokio::sync::{Notify, watch};

use super::keys::{Keys, Run, Stroke};
use super::{PasteKey, Pasted};

/// Text types offered and read back.
const MIME_TYPES: [&str; 3] = ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING"];
/// evdev key codes (linux/input-event-codes.h), which EIS keyboards take.
const KEY_LEFTCTRL: u32 = 29;
const KEY_LEFTSHIFT: u32 = 42;
const KEY_V: u32 = 47;
/// How long the focused app gets to ask for the text after the key press.
const PASTE_WAIT: Duration = Duration::from_millis(1500);
/// How long the clipboard takes to show us as its owner.
const OWNER_WAIT: Duration = Duration::from_millis(500);
/// How long the user gets to let go of the shortcut's modifiers (Ctrl, Shift) before typing.
const RELEASE_WAIT: Duration = Duration::from_secs(3);
/// Between typed characters, so apps keep up.
const KEY_GAP: Duration = Duration::from_millis(3);

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn explain(e: ashpd::Error) -> String {
    match e {
        ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled) => {
            "Pasting wasn't allowed (the desktop's dialog was closed).".into()
        }
        other => format!("Pasting isn't available: {other}"),
    }
}

/// What the clipboard looks like to the session.
#[derive(Debug, Default, Clone)]
struct Owner {
    /// The session owns the clipboard.
    ours: bool,
    mime_types: Vec<String>,
    /// Bumped on every change.
    changes: u64,
}

/// State shared with the background tasks.
#[derive(Default)]
struct Shared {
    /// The text the session offers when it owns the clipboard.
    offer: Mutex<String>,
    /// Transfers served (the focused app read the text).
    served: watch::Sender<u64>,
    owner: watch::Sender<Owner>,
    /// The EIS keyboard, once the compositor resumed it.
    keyboard: Mutex<Option<reis::event::Device>>,
    keyboard_ready: Notify,
    /// Characters to keys with the desktop's current layout.
    keys: Mutex<Keys>,
    /// Modifiers the user holds down (xkb mask), as the desktop reports them.
    held: watch::Sender<u32>,
    /// The EIS connection ended (the user stopped sharing, or the session closed).
    closed: AtomicBool,
}

/// A running auto-paste session.
pub struct AutoPaste {
    portal: RemoteDesktop,
    clipboard: Clipboard,
    session: Session<RemoteDesktop>,
    context: ei::Context,
    connection: reis::event::Connection,
    shared: Arc<Shared>,
    tasks: Vec<Task>,
    sequence: Mutex<u32>,
}

impl AutoPaste {
    /// Start the session; the first time (no token) the desktop asks the user. Returns the
    /// session and the token for the next start.
    pub async fn start(token: Option<&str>) -> Result<(Self, Option<String>), String> {
        let portal = RemoteDesktop::new().await.map_err(explain)?;
        let clipboard = Clipboard::new().await.map_err(explain)?;
        let session = portal
            .create_session(CreateSessionOptions::default())
            .await
            .map_err(explain)?;
        portal
            .select_devices(
                &session,
                SelectDevicesOptions::default()
                    .set_devices(Some(DeviceType::Keyboard.into()))
                    .set_persist_mode(PersistMode::ExplicitlyRevoked)
                    .set_restore_token(token),
            )
            .await
            .map_err(explain)?
            .response()
            .map_err(explain)?;
        // The clipboard must be requested before the session starts.
        clipboard
            .request(&session, Default::default())
            .await
            .map_err(explain)?;
        let started = portal
            .start(&session, None, StartOptions::default())
            .await
            .map_err(explain)?
            .response()
            .map_err(explain)?;
        if !started.devices().contains(DeviceType::Keyboard) {
            let _ = session.close().await;
            return Err("Pasting needs 'Allow Remote Interaction' in the desktop's dialog.".into());
        }
        if !started.is_clipboard_enabled() {
            let _ = session.close().await;
            return Err("Pasting needs 'Allow Clipboard Access' in the desktop's dialog.".into());
        }
        let new_token = started.restore_token().map(str::to_string);

        let fd: OwnedFd = portal
            .connect_to_eis(&session, ConnectToEISOptions::default())
            .await
            .map_err(explain)?;
        let shared = Arc::new(Shared::default());
        let (context, connection) = keyboard_thread(fd, shared.clone()).await?;
        let mut tasks = Vec::new();
        tasks.push(clipboard_task(shared.clone(), serve_transfers).await?);
        tasks.push(clipboard_task(shared.clone(), follow_owner).await?);
        let this = AutoPaste {
            portal,
            clipboard,
            session,
            context,
            connection,
            shared,
            tasks,
            sequence: Mutex::new(0),
        };
        // The keyboard arrives right after the handshake.
        if tokio::time::timeout(Duration::from_secs(5), this.wait_keyboard())
            .await
            .is_err()
        {
            this.close().await;
            return Err("Pasting isn't available: the desktop gave no keyboard.".into());
        }
        Ok((this, new_token))
    }

    async fn wait_keyboard(&self) {
        loop {
            let ready = self.shared.keyboard_ready.notified();
            if lock(&self.shared.keyboard).is_some() {
                return;
            }
            ready.await;
        }
    }

    /// The clipboard's current text, if it has any (not ours).
    async fn current_text(&self) -> Option<String> {
        let owner = self.shared.owner.borrow().clone();
        if owner.ours {
            return None;
        }
        let mime = MIME_TYPES
            .iter()
            .find(|m| owner.mime_types.iter().any(|o| o == *m))?;
        let fd = self
            .clipboard
            .selection_read(&self.session, mime)
            .await
            .ok()?;
        let read = tauri::async_runtime::spawn_blocking(move || {
            let mut text = String::new();
            std::fs::File::from(OwnedFd::from(fd))
                .read_to_string(&mut text)
                .ok()?;
            Some(text)
        });
        tokio::time::timeout(Duration::from_secs(1), read)
            .await
            .ok()?
            .ok()?
    }

    /// Offer `text` on the clipboard and wait until the clipboard shows us as its owner.
    async fn offer(&self, text: &str) -> Result<(), String> {
        *lock(&self.shared.offer) = text.to_string();
        let mut owner = self.shared.owner.subscribe();
        let before = owner.borrow_and_update().changes;
        self.clipboard
            .set_selection(
                &self.session,
                SetSelectionOptions::default().set_mime_types(&MIME_TYPES),
            )
            .await
            .map_err(explain)?;
        let _ = tokio::time::timeout(OWNER_WAIT, owner.wait_for(|o| o.changes > before && o.ours))
            .await;
        Ok(())
    }

    /// Hold `mods`, press and release `key` on the focused window (evdev codes).
    fn tap(&self, mods: &[u32], key: u32) -> Result<(), String> {
        let device = lock(&self.shared.keyboard)
            .clone()
            .ok_or("the keyboard went away")?;
        let keyboard = device
            .interface::<ei::Keyboard>()
            .ok_or("the keyboard went away")?;
        let dev = device.device();
        let mut sequence = lock(&self.sequence);
        *sequence += 1;
        dev.start_emulating(self.connection.serial(), *sequence);
        let now = || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_micros() as u64)
                .unwrap_or(0)
        };
        for &k in mods.iter().chain([&key]) {
            keyboard.key(k, ei::keyboard::KeyState::Press);
        }
        dev.frame(self.connection.serial(), now());
        for &k in mods.iter().chain([&key]).rev() {
            keyboard.key(k, ei::keyboard::KeyState::Released);
        }
        dev.frame(self.connection.serial(), now());
        dev.stop_emulating(self.connection.serial());
        self.context
            .flush()
            .map_err(|e| format!("Typing failed: {e}"))
    }

    /// Type the characters, one at a time.
    async fn type_strokes(&self, strokes: &[&Stroke]) -> Result<(), String> {
        for stroke in strokes {
            self.tap(&stroke.mods, stroke.key)?;
            tokio::time::sleep(KEY_GAP).await;
        }
        Ok(())
    }

    /// Paste `text` with the paste key; false when no app asked for it.
    async fn paste_run(&self, text: &str, key: PasteKey) -> Result<bool, String> {
        self.offer(text).await?;
        let mut served = self.shared.served.subscribe();
        let before = *served.borrow_and_update();
        match key {
            PasteKey::CtrlV => self.tap(&[KEY_LEFTCTRL], KEY_V)?,
            PasteKey::CtrlShiftV => self.tap(&[KEY_LEFTCTRL, KEY_LEFTSHIFT], KEY_V)?,
        }
        let pasted = tokio::time::timeout(PASTE_WAIT, served.wait_for(|n| *n > before))
            .await
            .is_ok();
        // Let the app insert it before the next typed character.
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(pasted)
    }

    /// Wait until the user has let go of the shortcut's modifiers: typing "a" while Ctrl is
    /// down would select everything.
    async fn wait_released(&self) {
        let mut held = self.shared.held.subscribe();
        let _ = tokio::time::timeout(RELEASE_WAIT, held.wait_for(|m| *m == 0)).await;
    }

    /// The EIS connection is still there.
    pub fn is_alive(&self) -> bool {
        !self.shared.closed.load(Ordering::SeqCst)
    }

    /// Type `text` into the focused window. Characters the layout can't type are pasted, and
    /// then the previous clipboard text is put back.
    pub async fn paste(&self, text: &str, key: PasteKey) -> Result<Pasted, String> {
        self.wait_released().await;
        let keys = lock(&self.shared.keys).clone();
        let runs = keys.runs(text);
        if !runs.iter().any(|r| matches!(r, Run::Paste(_))) {
            for run in &runs {
                if let Run::Type(strokes) = run {
                    self.type_strokes(strokes).await?;
                }
            }
            return Ok(Pasted::Typed);
        }
        let previous = self.current_text().await;
        for run in &runs {
            match run {
                Run::Type(strokes) => self.type_strokes(strokes).await?,
                Run::Paste(part) => {
                    if !self.paste_run(part, key).await? {
                        // Nothing asked for the text (no text field focused): offer all of it.
                        self.offer(text).await?;
                        return Ok(Pasted::NotAsked);
                    }
                }
            }
        }
        match previous {
            Some(prev) => {
                // Apps may read the text more than once right after the key press.
                tokio::time::sleep(Duration::from_millis(300)).await;
                self.offer(&prev).await?;
                Ok(Pasted::Restored)
            }
            None => {
                self.offer(text).await?;
                Ok(Pasted::Kept)
            }
        }
    }

    pub async fn close(self) {
        for t in &self.tasks {
            t.abort();
        }
        let _ = self.session.close().await;
        drop(self.portal);
    }
}

/// The EIS connection runs on its own thread: reis' event stream isn't `Send`. Returns once
/// the handshake is done; the thread follows the connection until it closes.
async fn keyboard_thread(
    fd: OwnedFd,
    shared: Arc<Shared>,
) -> Result<(ei::Context, reis::event::Connection), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("manent-paste-keys".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(Err(e.to_string()));
                    return;
                }
            };
            runtime.block_on(async move {
                let context = match ei::Context::new(UnixStream::from(fd)) {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(Err(e.to_string()));
                        return;
                    }
                };
                match context
                    .handshake_tokio("Manent", ei::handshake::ContextType::Sender)
                    .await
                {
                    Ok((connection, events)) => {
                        let _ = tx.send(Ok((context.clone(), connection)));
                        keyboard_events(events, context, shared.clone()).await;
                        shared.closed.store(true, Ordering::SeqCst);
                    }
                    Err(e) => {
                        let _ = tx.send(Err(format!("Pasting isn't available (keyboard): {e}")));
                    }
                }
            });
        })
        .map_err(|e| e.to_string())?;
    rx.await
        .map_err(|_| "the keyboard thread failed".to_string())?
}

/// Read the keymap the compositor sent with the keyboard (xkb text).
fn read_keymap(device: &reis::event::Device) -> Option<String> {
    let keymap = device.keymap()?;
    if keymap.type_ != ei::keyboard::KeymapType::Xkb {
        return None;
    }
    let file = std::fs::File::from(keymap.fd.try_clone().ok()?);
    let mut buf = vec![0; keymap.size as usize];
    file.read_exact_at(&mut buf, 0).ok()?;
    String::from_utf8(buf).ok()
}

/// Follow the EIS connection: bind the keyboard, keep track of it, its layout and the
/// modifiers the user holds.
async fn keyboard_events(
    mut events: reis::tokio::EiConvertEventStream,
    context: ei::Context,
    shared: Arc<Shared>,
) {
    let mut keymap: Option<String> = None;
    let (mut group, mut locked) = (0, 0);
    let rebuild = |keymap: &Option<String>, group: u32, locked: u32| {
        if let Some(keys) = keymap
            .as_deref()
            .and_then(|k| Keys::from_keymap(k, group, locked))
        {
            *lock(&shared.keys) = keys;
        }
    };
    while let Some(event) = events.next().await {
        let event = match event {
            Ok(e) => e,
            Err(e) => {
                eprintln!("autopaste: keyboard connection: {e}");
                break;
            }
        };
        match event {
            EiEvent::SeatAdded(added) => {
                added
                    .seat
                    .bind_capabilities(DeviceCapability::Keyboard.into());
                let _ = context.flush();
            }
            EiEvent::DeviceAdded(added)
                if added.device.has_capability(DeviceCapability::Keyboard) =>
            {
                keymap = read_keymap(&added.device);
                if keymap.is_none() {
                    eprintln!("autopaste: the keyboard came without a keymap");
                }
                rebuild(&keymap, group, locked);
            }
            EiEvent::DeviceResumed(resumed)
                if resumed.device.has_capability(DeviceCapability::Keyboard) =>
            {
                *lock(&shared.keyboard) = Some(resumed.device.clone());
                shared.keyboard_ready.notify_waiters();
            }
            EiEvent::KeyboardModifiers(m) => {
                shared.held.send_replace(m.depressed);
                if (m.group, m.locked) != (group, locked) {
                    (group, locked) = (m.group, m.locked);
                    rebuild(&keymap, group, locked);
                }
            }
            EiEvent::DevicePaused(_) | EiEvent::DeviceRemoved(_) => {
                *lock(&shared.keyboard) = None;
            }
            EiEvent::Disconnected(d) => {
                eprintln!("autopaste: keyboard disconnected: {:?}", d.reason);
                *lock(&shared.keyboard) = None;
                break;
            }
            _ => {}
        }
    }
}

type Task = tauri::async_runtime::JoinHandle<()>;

/// Run `f` with its own Clipboard proxy (its signal streams borrow it) on the app runtime;
/// returns once its streams are subscribed, so no signal is missed.
async fn clipboard_task<F, Fut>(shared: Arc<Shared>, f: F) -> Result<Task, String>
where
    F: FnOnce(Clipboard, Arc<Shared>, tokio::sync::oneshot::Sender<Result<(), String>>) -> Fut
        + Send
        + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let clipboard = Clipboard::new().await.map_err(explain)?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    let task = tauri::async_runtime::spawn(f(clipboard, shared, tx));
    rx.await
        .map_err(|_| "the clipboard task failed".to_string())??;
    Ok(task)
}

/// Keep track of who owns the clipboard and what it offers.
async fn follow_owner(
    clipboard: Clipboard,
    shared: Arc<Shared>,
    ready: tokio::sync::oneshot::Sender<Result<(), String>>,
) {
    let owners = match clipboard
        .receive_selection_owner_changed::<RemoteDesktop>()
        .await
    {
        Ok(s) => s,
        Err(e) => {
            let _ = ready.send(Err(explain(e)));
            return;
        }
    };
    let _ = ready.send(Ok(()));
    let mut owners = std::pin::pin!(owners);
    while let Some((_, change)) = owners.next().await {
        shared.owner.send_modify(|o| {
            o.ours = change.session_is_owner().unwrap_or(false);
            o.mime_types = change.mime_types().to_vec();
            o.changes += 1;
        });
    }
}

/// Hand the offered text to apps that paste it.
async fn serve_transfers(
    clipboard: Clipboard,
    shared: Arc<Shared>,
    ready: tokio::sync::oneshot::Sender<Result<(), String>>,
) {
    let transfers = match clipboard
        .receive_selection_transfer::<RemoteDesktop>()
        .await
    {
        Ok(s) => s,
        Err(e) => {
            let _ = ready.send(Err(explain(e)));
            return;
        }
    };
    let _ = ready.send(Ok(()));
    let mut transfers = std::pin::pin!(transfers);
    while let Some((session, _mime, serial)) = transfers.next().await {
        let text = lock(&shared.offer).clone();
        let ok = match clipboard.selection_write(&session, serial).await {
            Ok(fd) => tauri::async_runtime::spawn_blocking(move || {
                std::fs::File::from(OwnedFd::from(fd))
                    .write_all(text.as_bytes())
                    .is_ok()
            })
            .await
            .unwrap_or(false),
            Err(e) => {
                eprintln!("autopaste: clipboard transfer: {e}");
                false
            }
        };
        let _ = clipboard.selection_write_done(&session, serial, ok).await;
        shared.served.send_modify(|n| *n += 1);
    }
}
