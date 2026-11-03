//! Nextcloud sync (VRT-39, decision-5, decision-7): Veritas talks WebDAV to a folder on the
//! user's own Nextcloud, without the Nextcloud client apps.
//!
//! - **Connect:** Login Flow v2. The browser opens; Veritas polls for an app password that
//!   goes only into the OS keychain. Disconnect revokes it on the server.
//! - **Inbox:** recordings the phone puts in `<folder>/Inbox/` (with an optional
//!   `<name>.meta.json`: title and start time) are downloaded and imported (VRT-19). Once
//!   transcribed, the recording is moved on the server into the meeting's folder
//!   `<folder>/<YYYY-MM-DD HH.MM> <title>/` and `transcript.md` (VRT-37) is put next to it.
//! - **All meetings** (optional): desktop meetings get a folder too, with `mix.opus` and
//!   `transcript.md`. Never the database, never voice embeddings.
//! - **Changes:** a round costs one Depth-0 PROPFIND when nothing changed on the server;
//!   local transcripts are compared by hash and only changed ones go up. Rounds run every
//!   `POLL` and right away after a transcript is written, on "Sync now" and when the window
//!   comes to the front.
//!
//! Each file has one writer: the phone writes the inbox, the desktop the meeting folders.
//! A meeting folder deleted on the server is left alone (not deleted locally, not uploaded
//! again unasked).

pub mod dav;

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};
use tokio::sync::Notify;

use crate::db::Db;
use crate::jobs::{ImportMeta, Jobs};
use crate::media_server::MIX_FILE;
use crate::paths::AppPaths;
use dav::{Account, Dav};

pub const SERVER_SETTING: &str = "sync.server";
pub const LOGIN_SETTING: &str = "sync.login";
pub const FOLDER_SETTING: &str = "sync.folder";
pub const MODE_SETTING: &str = "sync.mode";
const KEYCHAIN_SERVICE: &str = "veritas-nextcloud";
const DEFAULT_FOLDER: &str = "Veritas";
/// Nextcloud's own `pollinterval` capability is 60 s by default.
const POLL: Duration = Duration::from_secs(60);
/// Audio and video the import understands (decision-3).
const MEDIA: [&str; 13] = [
    "opus", "ogg", "m4a", "mp3", "wav", "flac", "aac", "mp4", "mov", "mkv", "webm", "oga", "3gp",
];
/// Large files go up in chunks of this size (chunked upload v2).
const CHUNK: u64 = 10 * 1024 * 1024;

static WAKE: Notify = Notify::const_new();
/// The user's UTC offset in minutes, from the UI (folder names use local time).
static UTC_OFFSET_MIN: AtomicI32 = AtomicI32::new(0);
/// Rounds wait for the offset, so no folder is named in UTC by a round that ran before the UI.
static UTC_OFFSET_KNOWN: AtomicBool = AtomicBool::new(false);

/// Ask for a sync round now (e.g. a transcript was just written).
pub fn wake() {
    WAKE.notify_one();
}

pub fn set_utc_offset(minutes: i32) {
    UTC_OFFSET_MIN.store(minutes, Ordering::Relaxed);
    if !UTC_OFFSET_KNOWN.swap(true, Ordering::SeqCst) {
        wake();
    }
}

/// What Settings and the Library show.
#[derive(Debug, Clone, Serialize, Default)]
pub struct SyncStatus {
    /// "off", "connecting" (waiting for the browser), "idle", "syncing" or "error".
    pub state: &'static str,
    pub server: Option<String>,
    pub login: Option<String>,
    pub folder: String,
    /// "phone" (recordings from the phone only) or "all" (also every desktop meeting).
    pub mode: String,
    /// While connecting: the page the browser was sent to (to open it again).
    pub login_url: Option<String>,
    /// Unix ms of the last complete round.
    pub last_sync: Option<i64>,
    pub error: Option<String>,
    /// Plain http: the connection is not encrypted.
    pub insecure: bool,
}

struct Inner {
    app: Option<AppHandle>,
    db: Db,
    jobs: Jobs,
    paths: AppPaths,
    status: Mutex<SyncStatus>,
    /// Bumped to cancel a login that is still waiting for the browser.
    login_attempt: AtomicI32,
    /// The folder's ETag at the end of the last round; unchanged means nothing to fetch.
    root_etag: Mutex<Option<String>>,
    /// Inbox file -> import job, while the job runs.
    imports: Mutex<HashMap<String, u64>>,
}

#[derive(Clone)]
pub struct Sync {
    inner: Arc<Inner>,
}

type R<T> = Result<T, String>;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn keychain(server: &str, login: &str) -> R<keyring::Entry> {
    keyring::Entry::new(KEYCHAIN_SERVICE, &format!("{login}@{server}")).map_err(|e| e.to_string())
}

/// Run blocking WebDAV work off the async threads.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> R<T> + Send + 'static) -> R<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

impl Sync {
    pub fn start(app: Option<AppHandle>, db: Db, jobs: Jobs, paths: AppPaths) -> Self {
        let sync = Self {
            inner: Arc::new(Inner {
                app,
                db,
                jobs,
                paths,
                status: Mutex::new(SyncStatus {
                    state: "off",
                    folder: DEFAULT_FOLDER.into(),
                    mode: "phone".into(),
                    ..Default::default()
                }),
                login_attempt: AtomicI32::new(0),
                root_etag: Mutex::new(None),
                imports: Mutex::new(HashMap::new()),
            }),
        };
        let s = sync.clone();
        tauri::async_runtime::spawn(async move {
            s.load_status().await;
            loop {
                if UTC_OFFSET_KNOWN.load(Ordering::SeqCst) && s.account().await.is_some() {
                    s.round().await;
                }
                tokio::select! {
                    () = WAKE.notified() => {}
                    () = tokio::time::sleep(POLL) => {}
                }
            }
        });
        sync
    }

    pub fn status(&self) -> SyncStatus {
        lock(&self.inner.status).clone()
    }

    fn set_status(&self, f: impl FnOnce(&mut SyncStatus)) {
        let status = {
            let mut st = lock(&self.inner.status);
            f(&mut st);
            st.clone()
        };
        if let Some(app) = &self.inner.app {
            let _ = app.emit("sync:changed", status);
        }
    }

    async fn setting(&self, key: &str) -> Option<String> {
        self.inner
            .db
            .get_setting::<String>(key)
            .await
            .ok()
            .flatten()
    }

    async fn load_status(&self) {
        let (server, login) = (
            self.setting(SERVER_SETTING).await,
            self.setting(LOGIN_SETTING).await,
        );
        let folder = self
            .setting(FOLDER_SETTING)
            .await
            .unwrap_or_else(|| DEFAULT_FOLDER.into());
        let mode = self
            .setting(MODE_SETTING)
            .await
            .unwrap_or_else(|| "phone".into());
        self.set_status(|st| {
            st.state = if server.is_some() { "idle" } else { "off" };
            st.insecure = server.as_deref().is_some_and(|s| s.starts_with("http://"));
            st.server = server;
            st.login = login;
            st.folder = folder;
            st.mode = mode;
        });
    }

    /// The connected account, with its app password from the keychain.
    async fn account(&self) -> Option<Account> {
        let server = self.setting(SERVER_SETTING).await?;
        let login = self.setting(LOGIN_SETTING).await?;
        let (s, l) = (server.clone(), login.clone());
        let password = blocking(move || {
            keychain(&s, &l)?
                .get_password()
                .map_err(|e| format!("the app password is not in the keychain: {e}"))
        })
        .await;
        match password {
            Ok(app_password) => Some(Account {
                server,
                login,
                app_password,
            }),
            Err(e) => {
                self.set_status(|st| {
                    st.state = "error";
                    st.error = Some(e);
                });
                None
            }
        }
    }

    // ---- Connect and disconnect ----

    /// Start Login Flow v2 for `server`: returns the page to approve in the browser (opened
    /// too) and polls for the app password in the background.
    pub async fn connect(&self, server: String) -> R<String> {
        let server = normalize_server(&server)?;
        let s = server.clone();
        let (login_url, endpoint, token) = blocking(move || dav::start_login(&s)).await?;
        let attempt = self.inner.login_attempt.fetch_add(1, Ordering::SeqCst) + 1;
        self.set_status(|st| {
            st.state = "connecting";
            st.login_url = Some(login_url.clone());
            st.error = None;
        });
        crate::open_url::open(&login_url);
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            let started = std::time::Instant::now();
            while started.elapsed() < Duration::from_secs(20 * 60) {
                tokio::time::sleep(Duration::from_secs(2)).await;
                if me.inner.login_attempt.load(Ordering::SeqCst) != attempt {
                    return; // cancelled, or a newer attempt
                }
                let (e, t) = (endpoint.clone(), token.clone());
                match blocking(move || dav::poll_login(&e, &t)).await {
                    Ok(Some(acc)) => {
                        if let Err(e) = me.store_account(acc).await {
                            me.set_status(|st| {
                                st.state = "error";
                                st.error = Some(e);
                                st.login_url = None;
                            });
                        }
                        return;
                    }
                    Ok(None) => {}
                    Err(e) => eprintln!("sync: polling the login failed: {e}"),
                }
            }
            if me.inner.login_attempt.load(Ordering::SeqCst) == attempt {
                me.set_status(|st| {
                    st.state = "off";
                    st.login_url = None;
                    st.error = Some("The login was not approved in time.".into());
                });
            }
        });
        Ok(login_url)
    }

    async fn store_account(&self, acc: Account) -> R<()> {
        let (s, l, p) = (
            acc.server.clone(),
            acc.login.clone(),
            acc.app_password.clone(),
        );
        blocking(move || {
            keychain(&s, &l)?
                .set_password(&p)
                .map_err(|e| format!("keychain: {e}"))
        })
        .await?;
        let db = &self.inner.db;
        db.set_setting(SERVER_SETTING, &acc.server)
            .await
            .map_err(|e| e.to_string())?;
        db.set_setting(LOGIN_SETTING, &acc.login)
            .await
            .map_err(|e| e.to_string())?;
        *lock(&self.inner.root_etag) = None;
        self.load_status().await;
        self.set_status(|st| {
            st.login_url = None;
            st.error = None;
        });
        wake();
        Ok(())
    }

    pub fn cancel_connect(&self) {
        self.inner.login_attempt.fetch_add(1, Ordering::SeqCst);
        let connected = lock(&self.inner.status).server.is_some();
        self.set_status(|st| {
            st.state = if connected { "idle" } else { "off" };
            st.login_url = None;
        });
    }

    /// Revoke the app password on the server (best effort: the server may be gone) and
    /// forget it here, with all sync state. Files on the server and here stay.
    pub async fn disconnect(&self) -> R<()> {
        if let Some(acc) = self.account().await {
            let (s, l) = (acc.server.clone(), acc.login.clone());
            if let Err(e) = blocking(move || Dav::new(acc).revoke()).await {
                eprintln!("sync: revoking the app password failed: {e}");
            }
            let _ = blocking(move || {
                keychain(&s, &l)?
                    .delete_credential()
                    .map_err(|e| e.to_string())
            })
            .await;
        }
        let db = &self.inner.db;
        // The mode too: a new connection starts with the phone's recordings only, never by
        // uploading every meeting to a server the user connects later.
        for key in [SERVER_SETTING, LOGIN_SETTING, MODE_SETTING] {
            db.delete_setting(key).await.map_err(|e| e.to_string())?;
        }
        db.clear_sync().await.map_err(|e| e.to_string())?;
        *lock(&self.inner.root_etag) = None;
        self.load_status().await;
        self.set_status(|st| {
            st.last_sync = None;
            st.error = None;
        });
        Ok(())
    }

    pub async fn set_mode(&self, mode: &str) -> R<()> {
        if !matches!(mode, "phone" | "all") {
            return Err(format!("unknown sync mode {mode}"));
        }
        self.inner
            .db
            .set_setting(MODE_SETTING, &mode)
            .await
            .map_err(|e| e.to_string())?;
        self.set_status(|st| st.mode = mode.into());
        wake();
        Ok(())
    }

    /// Remove a meeting's folder on the server (the user deletes the meeting and chose to).
    pub async fn delete_remote(&self, meeting_id: i64) -> R<()> {
        let Some(row) = self
            .inner
            .db
            .sync_meeting(meeting_id)
            .await
            .map_err(|e| e.to_string())?
        else {
            return Ok(());
        };
        let acc = self.account().await.ok_or("not connected to Nextcloud")?;
        blocking(move || Dav::new(acc).delete(&row.remote_dir)).await
    }

    // ---- A sync round ----

    async fn round(&self) {
        let Some(acc) = self.account().await else {
            return;
        };
        self.set_status(|st| st.state = "syncing");
        let result = self.round_with(acc).await;
        let now = crate::commands::now_ms().ok();
        self.set_status(|st| match result {
            Ok(()) => {
                st.state = "idle";
                st.error = None;
                st.last_sync = now;
            }
            Err(e) => {
                eprintln!("sync: {e}");
                st.state = "error";
                st.error = Some(friendly(&e));
            }
        });
    }

    async fn round_with(&self, acc: Account) -> R<()> {
        let folder = lock(&self.inner.status).folder.clone();
        let inbox = format!("{folder}/Inbox");
        let dav = Dav::new(acc);
        // The folders (cheap when they exist: 405).
        {
            let (d, f, i) = (dav.clone(), folder.clone(), inbox.clone());
            blocking(move || {
                d.mkcol(&f)?;
                d.mkcol(&i)
            })
            .await?;
        }
        // Only list the inbox when something changed on the server.
        let etag = {
            let (d, f) = (dav.clone(), folder.clone());
            blocking(move || d.etag(&f)).await?
        };
        let changed = *lock(&self.inner.root_etag) != etag;
        if changed {
            self.fetch_inbox(&dav, &inbox).await?;
        }
        self.finish_imports(&dav, &folder).await?;
        if lock(&self.inner.status).mode == "all" {
            self.add_desktop_meetings(&dav, &folder).await?;
        }
        self.upload_changes(&dav).await?;
        // The ETag after our own writes, so they don't count as a change next round.
        let after = {
            let (d, f) = (dav.clone(), folder.clone());
            blocking(move || d.etag(&f)).await?
        };
        *lock(&self.inner.root_etag) = after;
        Ok(())
    }

    /// New recordings in the inbox: download and import each (once).
    async fn fetch_inbox(&self, dav: &Dav, inbox: &str) -> R<()> {
        let entries = {
            let (d, i) = (dav.clone(), inbox.to_string());
            blocking(move || d.propfind(&i, 1)).await?
        };
        let known: HashMap<String, (String, String)> = self
            .inner
            .db
            .inbox_items()
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|i| (i.remote_path, (i.state, i.etag)))
            .collect();
        for e in entries.iter().filter(|e| !e.is_dir && is_media(&e.path)) {
            // Known, unless an import failed and the phone uploaded the file again since.
            let retry = known
                .get(&e.path)
                .is_some_and(|(state, etag)| state == "failed" && *etag != e.etag);
            if (known.contains_key(&e.path) && !retry)
                || lock(&self.inner.imports).contains_key(&e.path)
            {
                continue;
            }
            let name = file_name(&e.path);
            let meta_path = format!("{}.meta.json", strip_ext(&e.path));
            let meta = if entries.iter().any(|m| m.path == meta_path) {
                self.read_meta(dav, &meta_path).await
            } else {
                ImportMeta::default()
            };
            let dir = self.inner.paths.tmp_dir().join("sync");
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let local = dir.join(sanitize(&name));
            {
                let (d, p, l) = (dav.clone(), e.path.clone(), local.clone());
                blocking(move || d.download(&p, &l)).await?;
            }
            let job = self.inner.jobs.import_as(local, name, meta);
            lock(&self.inner.imports).insert(e.path.clone(), job);
            self.inner
                .db
                .set_inbox_item(&e.path, &e.etag, None, "importing", None)
                .await
                .map_err(|e| e.to_string())?;
            eprintln!("sync: importing {}", e.path);
        }
        Ok(())
    }

    async fn read_meta(&self, dav: &Dav, path: &str) -> ImportMeta {
        let tmp = self.inner.paths.tmp_dir().join("sync-meta.json");
        let (d, p, t) = (dav.clone(), path.to_string(), tmp.clone());
        let text = blocking(move || {
            d.download(&p, &t)?;
            std::fs::read_to_string(&t).map_err(|e| e.to_string())
        })
        .await;
        let _ = std::fs::remove_file(&tmp);
        text.ok().map(|t| parse_meta(&t)).unwrap_or_default()
    }

    /// Imports that finished: move the recording into its meeting folder on the server.
    async fn finish_imports(&self, dav: &Dav, folder: &str) -> R<()> {
        let db = &self.inner.db;
        let jobs = self.inner.jobs.list();
        for item in db.inbox_items().await.map_err(|e| e.to_string())? {
            if item.state != "importing" {
                continue;
            }
            let job = lock(&self.inner.imports).get(&item.remote_path).copied();
            let job = job.and_then(|id| jobs.iter().find(|j| j.id == id));
            let meeting_id = item.meeting_id.or_else(|| job.and_then(|j| j.meeting_id));
            if let Some(j) = job
                && j.stage == "failed"
            {
                // A failed import removes its meeting again: don't point at it.
                db.set_inbox_item(
                    &item.remote_path,
                    &item.etag,
                    None,
                    "failed",
                    j.error.as_deref(),
                )
                .await
                .map_err(|e| e.to_string())?;
                lock(&self.inner.imports).remove(&item.remote_path);
                continue;
            }
            let Some(meeting_id) = meeting_id else {
                if job.is_none() {
                    // The app restarted before the import began: start over next round.
                    db.delete_inbox_item(&item.remote_path)
                        .await
                        .map_err(|e| e.to_string())?;
                    *lock(&self.inner.root_etag) = None;
                }
                continue;
            };
            if item.meeting_id.is_none() {
                db.set_inbox_item(
                    &item.remote_path,
                    &item.etag,
                    Some(meeting_id),
                    "importing",
                    None,
                )
                .await
                .map_err(|e| e.to_string())?;
            }
            let Some(meeting) = db
                .get_meeting(meeting_id)
                .await
                .map_err(|e| e.to_string())?
            else {
                continue;
            };
            if meeting.status != "done" {
                continue; // still transcribing
            }
            let remote_dir = self.new_meeting_dir(dav, folder, &meeting).await?;
            let name = file_name(&item.remote_path);
            let meta = format!("{}.meta.json", strip_ext(&item.remote_path));
            {
                let (d, from, rd) = (dav.clone(), item.remote_path.clone(), remote_dir.clone());
                blocking(move || {
                    d.mv(&from, &format!("{rd}/{name}"))?;
                    // Its meta.json goes along if there is one.
                    let meta_name = file_name(&meta);
                    let _ = d.mv(&meta, &format!("{rd}/{meta_name}"));
                    Ok(())
                })
                .await?;
            }
            db.add_sync_meeting(meeting_id, &remote_dir, "phone")
                .await
                .map_err(|e| e.to_string())?;
            db.set_inbox_item(
                &item.remote_path,
                &item.etag,
                Some(meeting_id),
                "done",
                None,
            )
            .await
            .map_err(|e| e.to_string())?;
            lock(&self.inner.imports).remove(&item.remote_path);
            let _ = std::fs::remove_file(
                self.inner
                    .paths
                    .tmp_dir()
                    .join("sync")
                    .join(sanitize(&file_name(&item.remote_path))),
            );
            eprintln!("sync: {} -> {remote_dir}", item.remote_path);
        }
        Ok(())
    }

    /// A fresh folder for a meeting: "<YYYY-MM-DD HH.MM> <title>", made unique.
    async fn new_meeting_dir(&self, dav: &Dav, folder: &str, m: &crate::db::Meeting) -> R<String> {
        let base = format!(
            "{folder}/{}",
            meeting_dir_name(
                m.started_at,
                &m.title,
                UTC_OFFSET_MIN.load(Ordering::Relaxed)
            )
        );
        let d = dav.clone();
        blocking(move || {
            for n in 1..100 {
                let dir = if n == 1 {
                    base.clone()
                } else {
                    format!("{base} ({n})")
                };
                if d.etag(&dir)?.is_none() {
                    d.mkcol(&dir)?;
                    return Ok(dir);
                }
            }
            Err("no free folder name".into())
        })
        .await
    }

    /// "All meetings": every finished desktop meeting without a folder gets one.
    async fn add_desktop_meetings(&self, dav: &Dav, folder: &str) -> R<()> {
        let db = &self.inner.db;
        let synced: Vec<i64> = db
            .sync_meetings()
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|s| s.meeting_id)
            .collect();
        let importing: Vec<i64> = db
            .inbox_items()
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter_map(|i| i.meeting_id)
            .collect();
        for m in db.list_meetings().await.map_err(|e| e.to_string())? {
            if m.status != "done" || synced.contains(&m.id) || importing.contains(&m.id) {
                continue;
            }
            let full = db.get_meeting(m.id).await.map_err(|e| e.to_string())?;
            let Some(meeting) = full else { continue };
            let dir = self.new_meeting_dir(dav, folder, &meeting).await?;
            db.add_sync_meeting(m.id, &dir, "desktop")
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Upload what changed locally: transcript.md (by hash), and for desktop meetings the
    /// playback file once.
    async fn upload_changes(&self, dav: &Dav) -> R<()> {
        let db = &self.inner.db;
        for row in db.sync_meetings().await.map_err(|e| e.to_string())? {
            if row.gone {
                continue;
            }
            let Some(meeting) = db
                .get_meeting(row.meeting_id)
                .await
                .map_err(|e| e.to_string())?
            else {
                continue;
            };
            let local = Path::new(&meeting.dir).join(crate::transcript_md::FILE);
            let text = std::fs::read(&local).ok();
            let hash = text.as_ref().map(|t| hex(&Sha256::digest(t)));
            let mut mix_uploaded = row.mix_uploaded;
            let mut uploaded_hash = row.transcript_hash.clone();
            if row.origin == "desktop" && !row.mix_uploaded {
                let mix = Path::new(&meeting.dir).join(MIX_FILE);
                if mix.is_file() {
                    let (d, rd) = (dav.clone(), row.remote_dir.clone());
                    match blocking(move || {
                        d.upload_chunked(
                            &mix,
                            &format!("{rd}/{MIX_FILE}"),
                            CHUNK,
                            None,
                            &mut |_| {},
                        )
                    })
                    .await
                    {
                        Ok(r) if r.finished => {
                            eprintln!(
                                "sync: {MIX_FILE} of meeting {}: {} bytes, {} of {} chunks sent ({} already there)",
                                row.meeting_id, r.size, r.sent, r.chunks, r.skipped
                            );
                            mix_uploaded = true;
                        }
                        Ok(_) => {}
                        Err(e) if gone(&e) => {
                            db.set_sync_gone(row.meeting_id)
                                .await
                                .map_err(|e| e.to_string())?;
                            continue;
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
            if let (Some(text), Some(hash)) = (text, hash.clone())
                && row.transcript_hash.as_deref() != Some(hash.as_str())
            {
                let (d, rd) = (dav.clone(), row.remote_dir.clone());
                match blocking(move || {
                    d.put(&format!("{rd}/{}", crate::transcript_md::FILE), &text)
                })
                .await
                {
                    Ok(()) => uploaded_hash = Some(hash),
                    Err(e) if gone(&e) => {
                        eprintln!(
                            "sync: {} was deleted on the server; leaving it",
                            row.remote_dir
                        );
                        db.set_sync_gone(row.meeting_id)
                            .await
                            .map_err(|e| e.to_string())?;
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            }
            if uploaded_hash != row.transcript_hash || mix_uploaded != row.mix_uploaded {
                db.set_sync_uploaded(row.meeting_id, uploaded_hash.as_deref(), mix_uploaded)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}

/// What the user reads for a failed round; the details go to the log.
fn friendly(e: &str) -> String {
    let l = e.to_ascii_lowercase();
    let offline = [
        "connection refused",
        "timed out",
        "timeout",
        "dns",
        "failed to lookup",
        "network is unreachable",
        "no route to host",
        "connection reset",
    ];
    if offline.iter().any(|k| l.contains(k)) {
        "Can't reach the server. Trying again every minute.".into()
    } else if l.contains("http 401") || l.contains("http 403") {
        "Nextcloud refused the app password. Disconnect and connect again.".into()
    } else if l.contains("http 507") {
        "The Nextcloud storage is full.".into()
    } else if l.contains("certificate") || l.contains("tls") {
        "The server's certificate isn't trusted by this computer.".into()
    } else {
        e.chars().take(200).collect()
    }
}

/// The server answered that the folder isn't there (anymore).
fn gone(e: &str) -> bool {
    e.contains("HTTP 404") || e.contains("HTTP 409")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// "nextcloud.example.org" -> "https://nextcloud.example.org"; no trailing slash.
pub fn normalize_server(s: &str) -> R<String> {
    let s = s.trim().trim_end_matches('/');
    if s.is_empty() {
        return Err("Enter your Nextcloud address.".into());
    }
    let s = if s.starts_with("http://") || s.starts_with("https://") {
        s.to_string()
    } else {
        format!("https://{s}")
    };
    if s.contains(' ') {
        return Err("That doesn't look like an address.".into());
    }
    Ok(s)
}

fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

fn strip_ext(path: &str) -> &str {
    match path.rfind('.') {
        Some(i) if i > path.rfind('/').map_or(0, |s| s + 1) => &path[..i],
        _ => path,
    }
}

fn is_media(path: &str) -> bool {
    let name = file_name(path);
    !name.starts_with('.')
        && name
            .rsplit_once('.')
            .is_some_and(|(_, ext)| MEDIA.contains(&ext.to_ascii_lowercase().as_str()))
}

/// A name that is safe as a local file or a folder name on any system.
fn sanitize(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if "/\\:*?\"<>|".contains(c) || c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let s = s.trim_matches('.').to_string();
    if s.is_empty() {
        "meeting".into()
    } else {
        s.chars().take(80).collect()
    }
}

/// "<YYYY-MM-DD HH.MM> <title>" in local time (the UTC offset from the UI).
pub fn meeting_dir_name(started_at_ms: i64, title: &str, utc_offset_min: i32) -> String {
    let local = started_at_ms / 1000 + i64::from(utc_offset_min) * 60;
    let iso = crate::export::iso_utc(local * 1000); // "YYYY-MM-DDTHH:MM:SSZ"
    let stamp = format!("{} {}", &iso[..10], iso[11..16].replace(':', "."));
    let title = sanitize(title.trim());
    if title.trim().is_empty() || title == "meeting" {
        stamp
    } else {
        format!("{stamp} {title}")
    }
}

/// A phone recording's meta.json: {"title": "...", "started_at": <unix ms>, "device": "..."}.
fn parse_meta(text: &str) -> ImportMeta {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    ImportMeta {
        title: v["title"]
            .as_str()
            .map(str::to_string)
            .filter(|t| !t.trim().is_empty()),
        started_at: v["started_at"].as_i64(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_read_as_sentences() {
        assert_eq!(
            friendly("MKCOL http://x/remote.php/dav: io: Connection refused (os error 111)"),
            "Can't reach the server. Trying again every minute."
        );
        assert!(friendly("PROPFIND: HTTP 401 <xml>").contains("app password"));
        assert!(friendly("PUT: HTTP 507").contains("full"));
        assert!(friendly("invalid peer certificate: UnknownIssuer").contains("certificate"));
        assert_eq!(friendly("something else"), "something else");
    }

    #[test]
    fn server_addresses_are_normalized() {
        assert_eq!(
            normalize_server(" cloud.example.org/ ").unwrap(),
            "https://cloud.example.org"
        );
        assert_eq!(
            normalize_server("http://127.0.0.1:8471").unwrap(),
            "http://127.0.0.1:8471"
        );
        assert!(normalize_server("").is_err());
        assert!(normalize_server("not an address").is_err());
    }

    #[test]
    fn only_media_files_are_picked_up_from_the_inbox() {
        assert!(is_media("Veritas/Inbox/rec 1.opus"));
        assert!(is_media("Veritas/Inbox/rec.M4A"));
        assert!(!is_media("Veritas/Inbox/rec.meta.json"));
        assert!(!is_media("Veritas/Inbox/.rec.opus.part"));
        assert!(!is_media("Veritas/Inbox/notes.txt"));
        assert_eq!(strip_ext("Veritas/Inbox/rec 1.opus"), "Veritas/Inbox/rec 1");
        assert_eq!(strip_ext("Veritas/In.box/rec"), "Veritas/In.box/rec");
    }

    #[test]
    fn meeting_folders_are_named_by_local_time_and_title() {
        // 2026-11-02 13:05 UTC, user in UTC+1.
        let t = 1_793_624_700_000;
        assert_eq!(
            meeting_dir_name(t, "Planning Q4", 60),
            "2026-11-02 14.05 Planning Q4"
        );
        assert_eq!(meeting_dir_name(t, "", 0), "2026-11-02 13.05");
        assert_eq!(meeting_dir_name(t, "a/b: c?", 0), "2026-11-02 13.05 a b c");
    }

    #[test]
    fn meta_json_gives_title_and_start() {
        let m = parse_meta(
            r#"{"title": "Klantgesprek", "started_at": 1793624700000, "device": "Pixel"}"#,
        );
        assert_eq!(
            (m.title.as_deref(), m.started_at),
            (Some("Klantgesprek"), Some(1_793_624_700_000))
        );
        let m = parse_meta("not json");
        assert_eq!((m.title, m.started_at), (None, None));
    }
}
