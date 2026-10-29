//! Tauri commands exposed to the UI. Keep in sync with `src/lib/ipc.ts`.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::audio::{self, Recording, SourceInfo, TrackSummary};

/// Basic app information, shown in Settings > About.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub os: String,
    pub arch: String,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "Veritas".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
    }
}

/// Microphone and system-audio (loopback) sources of this machine.
#[tauri::command]
pub fn list_sources() -> Result<Vec<SourceInfo>, String> {
    audio::list_sources().map_err(|e| e.to_string())
}

/// The recording in progress, if any.
#[derive(Default)]
pub struct RecorderState(Mutex<Option<Recording>>);

#[derive(Debug, Serialize)]
pub struct RecordingStarted {
    pub meeting_id: String,
    pub dir: String,
}

/// Start recording the given sources into a new meeting folder. Levels arrive as
/// `audio:level` events (~20 Hz per source).
#[tauri::command]
pub fn start_recording(
    app: AppHandle,
    state: State<'_, RecorderState>,
    source_ids: Vec<String>,
) -> Result<RecordingStarted, String> {
    let mut current = state.0.lock().map_err(|e| e.to_string())?;
    if current.is_some() {
        return Err("a recording is already running".into());
    }
    let available = audio::list_sources().map_err(|e| e.to_string())?;
    let sources: Vec<SourceInfo> = source_ids
        .iter()
        .map(|id| {
            available
                .iter()
                .find(|s| &s.id == id)
                .cloned()
                .ok_or_else(|| format!("unknown source {id}"))
        })
        .collect::<Result<_, _>>()?;
    // Meeting rows come with VRT-6/VRT-8; until then the id is the start time in ms.
    let meeting_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis()
        .to_string();
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("meetings")
        .join(&meeting_id);
    let events = app.clone();
    let recording = Recording::start(&dir, sources, move |level| {
        let _ = events.emit("audio:level", level);
    })
    .map_err(|e| e.to_string())?;
    *current = Some(recording);
    Ok(RecordingStarted {
        meeting_id,
        dir: dir.display().to_string(),
    })
}

#[tauri::command]
pub fn pause_recording(state: State<'_, RecorderState>) -> Result<(), String> {
    match state.0.lock().map_err(|e| e.to_string())?.as_ref() {
        Some(r) => {
            r.pause();
            Ok(())
        }
        None => Err("no recording".into()),
    }
}

#[tauri::command]
pub fn resume_recording(state: State<'_, RecorderState>) -> Result<(), String> {
    match state.0.lock().map_err(|e| e.to_string())?.as_ref() {
        Some(r) => {
            r.resume();
            Ok(())
        }
        None => Err("no recording".into()),
    }
}

/// Stop the recording, close the FLAC files and return what was written per track.
#[tauri::command]
pub fn stop_recording(state: State<'_, RecorderState>) -> Result<Vec<TrackSummary>, String> {
    let recording = state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or("no recording")?;
    recording.stop().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_reports_crate_version() {
        let info = app_info();
        assert_eq!(info.name, "Veritas");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert!(!info.os.is_empty());
    }

    #[test]
    fn app_info_serializes_with_ui_field_names() {
        let json = serde_json::to_value(app_info()).unwrap();
        for field in ["name", "version", "os", "arch"] {
            assert!(json.get(field).is_some(), "missing {field}");
        }
    }
}
