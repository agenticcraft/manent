//! Global keyboard shortcuts (VRT-40, decision-10): start or stop recording, mark a moment
//! (VRT-52) and hold to dictate (VRT-58, decision-11), also while the app is in the
//! background. On Linux through the XDG GlobalShortcuts portal; the desktop asks the user once
//! and keeps the keys, which the user changes in the desktop's settings. The recording keys and
//! dictation are each off until the user turns them on in Settings; one portal session binds
//! whichever are on.

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

/// Settings key: the user turned the recording shortcuts on.
pub const ENABLED_SETTING: &str = "shortcuts.enabled";
/// Settings key: the user turned dictation on (VRT-58).
pub const DICTATION_SETTING: &str = "dictation.enabled";
/// The app id the desktop knows us by (the installed `ai.agenticcraft.manent.desktop`).
pub const APP_ID: &str = "ai.agenticcraft.manent";
/// While a key is held the desktop repeats its activation (GNOME: after 500 ms, every 30 ms,
/// VRT-53). An activation after a longer silence is a new press whose release got lost.
const REPEAT_GAP: Duration = Duration::from_millis(700);

/// What a shortcut does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    ToggleRecording,
    MarkMoment,
    Dictate,
}

impl Action {
    pub const ALL: [Action; 3] = [Action::ToggleRecording, Action::MarkMoment, Action::Dictate];

    pub fn id(self) -> &'static str {
        match self {
            Action::ToggleRecording => "toggle-recording",
            Action::MarkMoment => "mark-moment",
            Action::Dictate => "dictate",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Action::ToggleRecording => "Start or stop recording",
            Action::MarkMoment => "Mark this moment",
            Action::Dictate => "Hold to dictate",
        }
    }

    /// The keys asked for (the desktop may give others, or the user may change them).
    pub fn preferred_trigger(self) -> &'static str {
        match self {
            Action::ToggleRecording => "CTRL+ALT+R",
            Action::MarkMoment => "CTRL+ALT+M",
            Action::Dictate => "CTRL+SHIFT+space",
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
    /// The recording shortcuts (start/stop, mark) are bound.
    pub enabled: bool,
    /// The dictation shortcut is bound (VRT-58).
    pub dictation: bool,
    /// The bound shortcuts, as the desktop reports them.
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
    // The usual order (Ctrl+Alt+Shift+Super), whatever order the desktop reports.
    let rank = |m: &str| {
        ["Ctrl", "Alt", "Shift", "Super"]
            .iter()
            .position(|x| *x == m)
    };
    parts.sort_by_key(|m| rank(m).unwrap_or(usize::MAX));
    let key = rest.trim();
    if !key.is_empty() {
        let mut chars = key.chars();
        let first = chars.next().map(|c| c.to_uppercase().collect::<String>());
        parts.push(first.unwrap_or_default() + chars.as_str());
    }
    Some(parts.join("+"))
}

/// What the desktop reports about a shortcut's key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Activated,
    Deactivated,
}

/// A key press or release, after taking out the repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Pressed,
    Released,
}

/// Turns the desktop's signals into presses and releases: the repeats while a key is held
/// are dropped (VRT-53). Once the desktop has sent a release, a key stays held until its
/// release (the repeats are not always regular). A desktop that never sends Deactivated still
/// gets one press per key press, as long as the presses are further apart than the repeats.
#[derive(Debug, Default)]
pub struct HoldTracker {
    /// Shortcuts held down, with the time of their last activation.
    held: HashMap<Action, Instant>,
    /// The desktop sends releases.
    releases: bool,
}

impl HoldTracker {
    pub fn signal(&mut self, action: Action, signal: Signal, now: Instant) -> Option<Edge> {
        match signal {
            Signal::Activated => {
                let repeat = self
                    .held
                    .get(&action)
                    .is_some_and(|last| self.releases || now.duration_since(*last) < REPEAT_GAP);
                self.held.insert(action, now);
                (!repeat).then_some(Edge::Pressed)
            }
            Signal::Deactivated => {
                self.releases = true;
                self.held.remove(&action).map(|_| Edge::Released)
            }
        }
    }
}

/// The app's shortcuts (managed state).
pub struct Shortcuts {
    status: Mutex<ShortcutStatus>,
    running: tokio::sync::Mutex<Option<platform::Running>>,
    hold: Arc<Mutex<HoldTracker>>,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Shortcuts {
            status: Mutex::new(ShortcutStatus {
                supported: platform::SUPPORTED,
                ..Default::default()
            }),
            running: tokio::sync::Mutex::new(None),
            hold: Arc::default(),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Run a shortcut's action: recording keys on the press, dictation on press and release.
fn on_signal(app: &AppHandle, hold: &Mutex<HoldTracker>, action: Action, signal: Signal) {
    let Some(edge) = lock(hold).signal(action, signal, Instant::now()) else {
        return;
    };
    eprintln!("shortcut: {} {edge:?}", action.id());
    match (action, edge) {
        (Action::ToggleRecording, Edge::Pressed) => crate::tray::toggle_recording(app.clone()),
        (Action::MarkMoment, Edge::Pressed) => crate::commands::mark_from_shortcut(app.clone()),
        (Action::Dictate, Edge::Pressed) => crate::dictation::pressed(app),
        (Action::Dictate, Edge::Released) => crate::dictation::released(app),
        _ => {}
    }
}

/// The shortcuts to bind for the recording keys and dictation.
fn wanted(recording: bool, dictation: bool) -> Vec<Action> {
    Action::ALL
        .into_iter()
        .filter(|a| match a {
            Action::Dictate => dictation,
            _ => recording,
        })
        .collect()
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

    /// Bind the recording shortcuts and/or dictation and listen for them; with neither, let
    /// go of the keys. The first time a shortcut is bound the desktop asks the user, so this
    /// can take as long as they take. If that fails, what was bound before is bound again.
    pub async fn apply(&self, app: &AppHandle, recording: bool, dictation: bool) -> ShortcutStatus {
        let mut running = self.running.lock().await;
        let before = self.status();
        if let Some(r) = running.take() {
            r.stop().await;
        }
        let attempts = [(recording, dictation), (before.enabled, before.dictation)];
        let mut error = None;
        for (i, (rec, dict)) in attempts.into_iter().enumerate() {
            if i > 0 && (rec, dict) == (recording, dictation) {
                break;
            }
            let actions = wanted(rec, dict);
            if actions.is_empty() {
                return self.set_status(app, |s| {
                    s.enabled = false;
                    s.dictation = false;
                    s.shortcuts.clear();
                    s.error = error.clone();
                });
            }
            let (on_action, on_changed) = {
                let (a1, a2, hold) = (app.clone(), app.clone(), self.hold.clone());
                (
                    move |action: Action, signal: Signal| on_signal(&a1, &hold, action, signal),
                    move |views: Vec<ShortcutView>| {
                        if let Some(s) = a2.try_state::<Shortcuts>() {
                            s.set_status(&a2, |st| st.shortcuts = views);
                        }
                    },
                )
            };
            match platform::bind(&actions, on_action, on_changed).await {
                Ok((r, views)) => {
                    *running = Some(r);
                    return self.set_status(app, |s| {
                        s.enabled = rec;
                        s.dictation = dict;
                        s.shortcuts = views;
                        s.error = error.clone();
                    });
                }
                Err(e) => {
                    eprintln!("shortcuts: {e}");
                    error.get_or_insert(e);
                }
            }
        }
        self.set_status(app, |s| {
            s.enabled = false;
            s.dictation = false;
            s.shortcuts.clear();
            s.error = error;
        })
    }
}

/// At start: listen again if the user turned the shortcuts or dictation on earlier.
pub fn start(app: &AppHandle, db: &Db) {
    let (app, db) = (app.clone(), db.clone());
    tauri::async_runtime::spawn(async move {
        let on = |key| {
            let db = db.clone();
            async move { db.get_setting::<bool>(key).await.ok().flatten() == Some(true) }
        };
        let (recording, dictation) = (on(ENABLED_SETTING).await, on(DICTATION_SETTING).await);
        if (recording || dictation) && platform::SUPPORTED {
            app.state::<Shortcuts>()
                .apply(&app, recording, dictation)
                .await;
        }
    });
}

async fn save(db: &Db, status: &ShortcutStatus) -> Result<(), String> {
    db.set_setting(ENABLED_SETTING, &status.enabled)
        .await
        .map_err(|e| e.to_string())?;
    db.set_setting(DICTATION_SETTING, &status.dictation)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_shortcuts(shortcuts: State<'_, Shortcuts>) -> Result<ShortcutStatus, String> {
    Ok(shortcuts.status())
}

/// Turn the recording shortcuts on (the desktop may ask the user first) or off.
#[tauri::command]
pub async fn set_shortcuts_enabled(
    app: AppHandle,
    shortcuts: State<'_, Shortcuts>,
    db: State<'_, Db>,
    enabled: bool,
) -> Result<ShortcutStatus, String> {
    let dictation = shortcuts.status().dictation;
    let status = shortcuts.apply(&app, enabled, dictation).await;
    save(&db, &status).await?;
    Ok(status)
}

/// Turn dictation on (the desktop may ask the user first) or off (VRT-58).
#[tauri::command]
pub async fn set_dictation_enabled(
    app: AppHandle,
    shortcuts: State<'_, Shortcuts>,
    db: State<'_, Db>,
    enabled: bool,
) -> Result<ShortcutStatus, String> {
    let recording = shortcuts.status().enabled;
    let status = shortcuts.apply(&app, recording, enabled).await;
    save(&db, &status).await?;
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
        assert_eq!(
            pretty_trigger("Press <Shift><Control>space").as_deref(),
            Some("Ctrl+Shift+Space")
        );
        assert_eq!(pretty_trigger("").as_deref(), None);
        assert_eq!(pretty_trigger("Press ").as_deref(), None);
    }

    fn ms(start: Instant, ms: u64) -> Instant {
        start + Duration::from_millis(ms)
    }

    #[test]
    fn a_held_key_is_one_press_and_one_release() {
        let mut t = HoldTracker::default();
        let t0 = Instant::now();
        let d = Action::Dictate;
        assert_eq!(t.signal(d, Signal::Activated, t0), Some(Edge::Pressed));
        // GNOME repeats after 500 ms, every 30 ms.
        for i in 0..100 {
            assert_eq!(t.signal(d, Signal::Activated, ms(t0, 500 + 30 * i)), None);
        }
        assert_eq!(
            t.signal(d, Signal::Deactivated, ms(t0, 3600)),
            Some(Edge::Released)
        );
        assert_eq!(t.signal(d, Signal::Deactivated, ms(t0, 3601)), None);
        // The next press counts again, however soon.
        assert_eq!(
            t.signal(d, Signal::Activated, ms(t0, 3700)),
            Some(Edge::Pressed)
        );
    }

    #[test]
    fn without_releases_presses_further_apart_than_repeats_still_count() {
        let mut t = HoldTracker::default();
        let t0 = Instant::now();
        let a = Action::ToggleRecording;
        assert_eq!(t.signal(a, Signal::Activated, t0), Some(Edge::Pressed));
        assert_eq!(t.signal(a, Signal::Activated, ms(t0, 300)), None);
        assert_eq!(
            t.signal(a, Signal::Activated, ms(t0, 2000)),
            Some(Edge::Pressed)
        );
    }

    #[test]
    fn shortcuts_are_tracked_separately() {
        let mut t = HoldTracker::default();
        let t0 = Instant::now();
        assert_eq!(
            t.signal(Action::Dictate, Signal::Activated, t0),
            Some(Edge::Pressed)
        );
        assert_eq!(
            t.signal(Action::MarkMoment, Signal::Activated, t0),
            Some(Edge::Pressed)
        );
        assert_eq!(
            t.signal(Action::MarkMoment, Signal::Deactivated, t0),
            Some(Edge::Released)
        );
        assert_eq!(
            t.signal(Action::Dictate, Signal::Activated, ms(t0, 600)),
            None
        );
    }

    #[test]
    fn only_the_shortcuts_turned_on_are_bound() {
        assert_eq!(wanted(false, false), []);
        assert_eq!(wanted(false, true), [Action::Dictate]);
        assert_eq!(
            wanted(true, false),
            [Action::ToggleRecording, Action::MarkMoment]
        );
        assert_eq!(wanted(true, true), Action::ALL);
    }

    #[test]
    fn actions_round_trip_through_their_ids() {
        for a in Action::ALL {
            assert_eq!(Action::from_id(a.id()), Some(a));
        }
        assert_eq!(Action::from_id("nope"), None);
    }
}
