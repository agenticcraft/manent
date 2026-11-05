//! Global keyboard shortcuts (VRT-40, decision-10): start or stop recording and mark a moment
//! (VRT-52), also while the app is in the background. On Linux through the XDG
//! GlobalShortcuts portal; the desktop asks the user once and keeps the keys, which the user
//! changes in the desktop's settings. Off until the user turns them on in Settings.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::db::Db;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(not(target_os = "linux"))]
mod other;
#[cfg(not(target_os = "linux"))]
use other as platform;

/// Settings key: the user turned the shortcuts on.
pub const ENABLED_SETTING: &str = "shortcuts.enabled";
/// The app id the desktop knows us by (the installed `ai.agenticcraft.manent.desktop`).
pub const APP_ID: &str = "ai.agenticcraft.manent";
/// A second activation of the same shortcut this soon is the same key press (key repeat).
const DEBOUNCE: Duration = Duration::from_millis(700);

/// What a shortcut does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    ToggleRecording,
    MarkMoment,
}

impl Action {
    pub const ALL: [Action; 2] = [Action::ToggleRecording, Action::MarkMoment];

    pub fn id(self) -> &'static str {
        match self {
            Action::ToggleRecording => "toggle-recording",
            Action::MarkMoment => "mark-moment",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Action::ToggleRecording => "Start or stop recording",
            Action::MarkMoment => "Mark this moment",
        }
    }

    /// The keys asked for (the desktop may give others, or the user may change them).
    pub fn preferred_trigger(self) -> &'static str {
        match self {
            Action::ToggleRecording => "CTRL+ALT+R",
            Action::MarkMoment => "CTRL+ALT+M",
        }
    }

    pub fn from_id(id: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.id() == id)
    }
}

/// A shortcut as Settings shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ShortcutView {
    pub id: String,
    pub description: String,
    /// "Ctrl+Alt+R"; None when the user gave it no keys.
    pub trigger: Option<String>,
}

/// The shortcuts' state for Settings (event `shortcuts:changed`).
#[derive(Debug, Clone, Default, Serialize)]
pub struct ShortcutStatus {
    /// This desktop offers global shortcuts.
    pub supported: bool,
    pub enabled: bool,
    pub shortcuts: Vec<ShortcutView>,
    /// Why they could not be turned on.
    pub error: Option<String>,
}

/// "Press <Control><Alt>r" (GNOME's trigger description) as "Ctrl+Alt+R".
pub fn pretty_trigger(description: &str) -> Option<String> {
    let s = description.trim();
    let s = s.strip_prefix("Press").unwrap_or(s).trim();
    if s.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    let mut rest = s;
    while let Some(after) = rest.strip_prefix('<') {
        let Some(end) = after.find('>') else { break };
        parts.push(match &after[..end] {
            "Control" | "Primary" | "Ctrl" => "Ctrl".to_string(),
            "Alt" => "Alt".into(),
            "Shift" => "Shift".into(),
            "Super" | "Meta" => "Super".into(),
            other => other.to_string(),
        });
        rest = &after[end + 1..];
    }
    let key = rest.trim();
    if !key.is_empty() {
        let mut chars = key.chars();
        let first = chars.next().map(|c| c.to_uppercase().collect::<String>());
        parts.push(first.unwrap_or_default() + chars.as_str());
    }
    Some(parts.join("+"))
}

/// The app's shortcuts (managed state).
pub struct Shortcuts {
    status: Mutex<ShortcutStatus>,
    running: tokio::sync::Mutex<Option<platform::Running>>,
    last: Arc<Mutex<HashMap<Action, Instant>>>,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Shortcuts {
            status: Mutex::new(ShortcutStatus {
                supported: platform::SUPPORTED,
                ..Default::default()
            }),
            running: tokio::sync::Mutex::new(None),
            last: Arc::default(),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Run a shortcut's action (once per key press).
fn activate(app: &AppHandle, last: &Mutex<HashMap<Action, Instant>>, action: Action) {
    {
        let mut last = lock(last);
        let now = Instant::now();
        if last.get(&action).is_some_and(|t| now - *t < DEBOUNCE) {
            return;
        }
        last.insert(action, now);
    }
    eprintln!("shortcut: {}", action.id());
    match action {
        Action::ToggleRecording => crate::tray::toggle_recording(app.clone()),
        Action::MarkMoment => crate::commands::mark_from_shortcut(app.clone()),
    }
}

impl Shortcuts {
    pub fn status(&self) -> ShortcutStatus {
        lock(&self.status).clone()
    }

    fn set_status(&self, app: &AppHandle, f: impl FnOnce(&mut ShortcutStatus)) -> ShortcutStatus {
        let status = {
            let mut s = lock(&self.status);
            f(&mut s);
            s.clone()
        };
        let _ = app.emit("shortcuts:changed", &status);
        status
    }

    /// Bind the shortcuts and listen for them. The first time the desktop asks the user,
    /// so this can take as long as they take.
    pub async fn enable(&self, app: &AppHandle) -> ShortcutStatus {
        let mut running = self.running.lock().await;
        if let Some(r) = running.take() {
            r.stop().await;
        }
        let (on_action, on_changed) = {
            let (a1, a2, last) = (app.clone(), app.clone(), self.last.clone());
            (
                move |action: Action| activate(&a1, &last, action),
                move |views: Vec<ShortcutView>| {
                    if let Some(s) = a2.try_state::<Shortcuts>() {
                        s.set_status(&a2, |st| st.shortcuts = views);
                    }
                },
            )
        };
        match platform::bind(on_action, on_changed).await {
            Ok((r, views)) => {
                *running = Some(r);
                self.set_status(app, |s| {
                    s.enabled = true;
                    s.shortcuts = views;
                    s.error = None;
                })
            }
            Err(e) => {
                eprintln!("shortcuts: {e}");
                self.set_status(app, |s| {
                    s.enabled = false;
                    s.shortcuts.clear();
                    s.error = Some(e);
                })
            }
        }
    }

    pub async fn disable(&self, app: &AppHandle) -> ShortcutStatus {
        if let Some(r) = self.running.lock().await.take() {
            r.stop().await;
        }
        self.set_status(app, |s| {
            s.enabled = false;
            s.shortcuts.clear();
            s.error = None;
        })
    }
}

/// At start: listen again if the user turned the shortcuts on earlier.
pub fn start(app: &AppHandle, db: &Db) {
    let (app, db) = (app.clone(), db.clone());
    tauri::async_runtime::spawn(async move {
        let on: Option<bool> = db.get_setting(ENABLED_SETTING).await.ok().flatten();
        if on == Some(true) && platform::SUPPORTED {
            app.state::<Shortcuts>().enable(&app).await;
        }
    });
}

#[tauri::command]
pub async fn get_shortcuts(shortcuts: State<'_, Shortcuts>) -> Result<ShortcutStatus, String> {
    Ok(shortcuts.status())
}

/// Turn the shortcuts on (the desktop may ask the user first) or off.
#[tauri::command]
pub async fn set_shortcuts_enabled(
    app: AppHandle,
    shortcuts: State<'_, Shortcuts>,
    db: State<'_, Db>,
    enabled: bool,
) -> Result<ShortcutStatus, String> {
    let status = if enabled {
        shortcuts.enable(&app).await
    } else {
        shortcuts.disable(&app).await
    };
    db.set_setting(ENABLED_SETTING, &status.enabled)
        .await
        .map_err(|e| e.to_string())?;
    Ok(status)
}

/// Open the desktop's settings where the user changes the keys.
#[tauri::command]
pub async fn open_shortcut_settings() -> Result<(), String> {
    platform::open_settings()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gnome_trigger_descriptions_read_like_keys() {
        assert_eq!(
            pretty_trigger("Press <Control><Alt>r").as_deref(),
            Some("Ctrl+Alt+R")
        );
        assert_eq!(
            pretty_trigger("<Shift><Super>F9").as_deref(),
            Some("Shift+Super+F9")
        );
        assert_eq!(pretty_trigger("").as_deref(), None);
        assert_eq!(pretty_trigger("Press ").as_deref(), None);
    }

    #[test]
    fn actions_round_trip_through_their_ids() {
        for a in Action::ALL {
            assert_eq!(Action::from_id(a.id()), Some(a));
        }
        assert_eq!(Action::from_id("nope"), None);
    }
}
