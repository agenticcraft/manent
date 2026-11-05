//! Global shortcuts through the XDG GlobalShortcuts portal (decision-10).

use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut, Shortcut};
use futures_util::StreamExt;

use super::{APP_ID, Action, ShortcutView, pretty_trigger};

pub const SUPPORTED: bool = true;

/// A bound session; stop it to let go of the keys.
pub struct Running {
    stop: tokio::sync::oneshot::Sender<()>,
    task: tauri::async_runtime::JoinHandle<()>,
}

impl Running {
    pub async fn stop(self) {
        let _ = self.stop.send(());
        let _ = self.task.await;
    }
}

fn views(shortcuts: &[Shortcut]) -> Vec<ShortcutView> {
    shortcuts
        .iter()
        .map(|s| ShortcutView {
            id: s.id().to_string(),
            description: s.description().to_string(),
            trigger: pretty_trigger(s.trigger_description()),
        })
        .collect()
}

fn explain(e: ashpd::Error) -> String {
    match e {
        ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)
        | ashpd::Error::Response(ashpd::desktop::ResponseError::Other) => {
            "The shortcuts were not added (the desktop's dialog was closed).".into()
        }
        ashpd::Error::Portal(ashpd::PortalError::NotAllowed(_)) => {
            "This desktop does not know Manent's app id; install Manent from its package.".into()
        }
        other => format!("Global shortcuts are not available: {other}"),
    }
}

pub async fn bind(
    on_action: impl Fn(Action) + Send + Sync + 'static,
    on_changed: impl Fn(Vec<ShortcutView>) + Send + Sync + 'static,
) -> Result<(Running, Vec<ShortcutView>), String> {
    // A host app (not sandboxed) tells the portal who it is; this needs the desktop file
    // named after the id. Without it the portal refuses the session below.
    let id = ashpd::AppID::try_from(APP_ID).map_err(|e| e.to_string())?;
    if let Err(e) = ashpd::register_host_app(id).await {
        eprintln!("shortcuts: registering {APP_ID} failed: {e}");
    }
    let portal = GlobalShortcuts::new().await.map_err(explain)?;
    let session = portal
        .create_session(Default::default())
        .await
        .map_err(explain)?;
    let wanted: Vec<NewShortcut> = Action::ALL
        .iter()
        .map(|a| NewShortcut::new(a.id(), a.description()).preferred_trigger(a.preferred_trigger()))
        .collect();
    let bound = portal
        .bind_shortcuts(&session, &wanted, None, Default::default())
        .await
        .map_err(explain)?
        .response()
        .map_err(explain)?;
    let mut activated = portal.receive_activated().await.map_err(explain)?;
    let mut changed = portal.receive_shortcuts_changed().await.map_err(explain)?;
    let (stop, mut stopped) = tokio::sync::oneshot::channel();
    let task = tauri::async_runtime::spawn(async move {
        // The portal sends a session's signals to its owner only, and this process has one
        // session at a time, so every signal here is ours.
        loop {
            tokio::select! {
                Some(a) = activated.next() => {
                    if let Some(action) = Action::from_id(a.shortcut_id()) {
                        on_action(action);
                    }
                }
                Some(c) = changed.next() => on_changed(views(c.shortcuts())),
                _ = &mut stopped => break,
            }
        }
        let _ = session.close().await;
        drop(portal);
    });
    Ok((Running { stop, task }, views(bound.shortcuts())))
}

/// GNOME Settings > Apps > Manent, where the keys are changed (portal version 1 has no
/// ConfigureShortcuts).
pub fn open_settings() -> Result<(), String> {
    std::process::Command::new("gnome-control-center")
        .args(["applications", APP_ID])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open the desktop's settings: {e}"))
}
