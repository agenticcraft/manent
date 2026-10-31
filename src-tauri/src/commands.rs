//! Tauri commands exposed to the UI. Keep in sync with `src/lib/ipc.ts`.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::audio::{self, Monitor, Recording, SourceInfo, SourceKind, TrackSummary};
use crate::db::{Db, Meeting, NewTrack, Speaker};
use crate::engine::{EngineManager, EngineStatus};
use crate::media_server::{MIX_FILE, MediaServer};
use crate::paths::AppPaths;

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

/// Settings key holding the source ids to record (set in Settings > Sources, VRT-9).
pub const SOURCES_SETTING: &str = "recording.sources";

struct ActiveRecording {
    recording: Recording,
    meeting_id: i64,
    started_at: i64,
    sources: Vec<SourceInfo>,
}

/// The recording in progress, if any.
#[derive(Default)]
pub struct RecorderState(tokio::sync::Mutex<Option<ActiveRecording>>);

#[derive(Debug, Clone, Serialize)]
pub struct RecordingStarted {
    pub meeting_id: i64,
    /// Unix epoch milliseconds.
    pub started_at: i64,
    pub sources: Vec<SourceInfo>,
}

#[derive(Debug, Serialize)]
pub struct RecordingStopped {
    pub meeting_id: i64,
    pub tracks: Vec<TrackSummary>,
    /// The playback mixdown, or None if it could not be made (the tracks are still kept).
    pub mix: Option<String>,
}

/// The configured sources that are available now; without a usable configuration the
/// system default microphone and output (or the first of each kind).
pub fn resolve_sources(available: &[SourceInfo], configured: Option<&[String]>) -> Vec<SourceInfo> {
    let chosen: Vec<SourceInfo> = configured
        .unwrap_or_default()
        .iter()
        .filter_map(|id| available.iter().find(|s| &s.id == id).cloned())
        .collect();
    if !chosen.is_empty() {
        return chosen;
    }
    [SourceKind::Mic, SourceKind::Loopback]
        .into_iter()
        .filter_map(|kind| {
            available
                .iter()
                .find(|s| s.kind == kind && s.is_default)
                .or_else(|| available.iter().find(|s| s.kind == kind))
                .cloned()
        })
        .collect()
}

fn now_ms() -> Result<i64, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as i64)
}

/// The source choice shown in Settings > Sources.
#[derive(Debug, Serialize)]
pub struct SourceSettings {
    pub available: Vec<SourceInfo>,
    /// Microphone to record (always one).
    pub mic: Option<String>,
    /// System audio to record, or None for "don't record system audio".
    pub system: Option<String>,
}

/// Split the effective source choice into the mic and system picks. An explicit empty
/// system choice is stored as the configured list without a loopback source.
pub fn source_settings(
    available: Vec<SourceInfo>,
    configured: Option<&[String]>,
) -> SourceSettings {
    let chosen = resolve_sources(&available, configured);
    let pick = |kind| chosen.iter().find(|s| s.kind == kind).map(|s| s.id.clone());
    SourceSettings {
        mic: pick(SourceKind::Mic),
        system: pick(SourceKind::Loopback),
        available,
    }
}

#[tauri::command]
pub async fn get_recording_sources(db: State<'_, Db>) -> Result<SourceSettings, String> {
    let available = audio::list_sources().map_err(|e| e.to_string())?;
    let configured: Option<Vec<String>> = db
        .get_setting(SOURCES_SETTING)
        .await
        .map_err(|e| e.to_string())?;
    Ok(source_settings(available, configured.as_deref()))
}

/// Save which microphone and (optionally) which system-audio source to record.
#[tauri::command]
pub async fn set_recording_sources(
    db: State<'_, Db>,
    mic: String,
    system: Option<String>,
) -> Result<(), String> {
    let ids: Vec<String> = std::iter::once(mic).chain(system).collect();
    db.set_setting(SOURCES_SETTING, &ids)
        .await
        .map_err(|e| e.to_string())
}

/// The level monitor used by Settings > Sources, if running.
#[derive(Default)]
pub struct MonitorState(tokio::sync::Mutex<Option<Monitor>>);

/// Start (or restart) monitoring the given sources; levels arrive as `audio:monitor-level`.
#[tauri::command]
pub async fn start_source_monitor(
    app: AppHandle,
    monitor: State<'_, MonitorState>,
    recorder: State<'_, RecorderState>,
    source_ids: Vec<String>,
) -> Result<(), String> {
    if recorder.0.lock().await.is_some() {
        return Err("a recording is running".into());
    }
    let mut current = monitor.0.lock().await;
    current.take(); // stop the previous monitor first
    let available = audio::list_sources().map_err(|e| e.to_string())?;
    let sources: Vec<SourceInfo> = source_ids
        .iter()
        .filter_map(|id| available.iter().find(|s| &s.id == id).cloned())
        .collect();
    if sources.is_empty() {
        return Ok(());
    }
    let events = app.clone();
    let m = Monitor::start(sources, move |level| {
        let _ = events.emit("audio:monitor-level", level);
    })
    .map_err(|e| e.to_string())?;
    *current = Some(m);
    Ok(())
}

#[tauri::command]
pub async fn stop_source_monitor(monitor: State<'_, MonitorState>) -> Result<(), String> {
    monitor.0.lock().await.take();
    Ok(())
}

/// Create a meeting and start recording the configured sources into its folder. Levels
/// arrive as `audio:level` events (~20 Hz per source).
#[tauri::command]
pub async fn start_recording(
    app: AppHandle,
    state: State<'_, RecorderState>,
    monitor: State<'_, MonitorState>,
    db: State<'_, Db>,
    paths: State<'_, AppPaths>,
) -> Result<RecordingStarted, String> {
    let mut current = state.0.lock().await;
    if current.is_some() {
        return Err("a recording is already running".into());
    }
    monitor.0.lock().await.take(); // free the devices used by Settings' test meters
    let available = audio::list_sources().map_err(|e| e.to_string())?;
    let configured: Option<Vec<String>> = db
        .get_setting(SOURCES_SETTING)
        .await
        .map_err(|e| e.to_string())?;
    let sources = resolve_sources(&available, configured.as_deref());
    if sources.is_empty() {
        return Err("no audio sources found".into());
    }
    let started_at = now_ms()?;
    // The meeting row comes first so its folder can be named after the database id.
    let pending = paths.meetings_root().join(format!("pending-{started_at}"));
    let meeting_id = db
        .create_meeting("", started_at, &pending.display().to_string())
        .await
        .map_err(|e| e.to_string())?;
    let dir = paths.meetings_root().join(meeting_id.to_string());
    db.set_meeting_dir(meeting_id, &dir.display().to_string())
        .await
        .map_err(|e| e.to_string())?;
    let events = app.clone();
    let recording = match Recording::start(&dir, sources.clone(), move |level| {
        let _ = events.emit("audio:level", level);
    }) {
        Ok(r) => r,
        Err(e) => {
            let _ = db.delete_meeting(meeting_id).await;
            let _ = std::fs::remove_dir_all(&dir);
            return Err(e.to_string());
        }
    };
    *current = Some(ActiveRecording {
        recording,
        meeting_id,
        started_at,
        sources: sources.clone(),
    });
    Ok(RecordingStarted {
        meeting_id,
        started_at,
        sources,
    })
}

/// The recording in progress (e.g. after the UI reloads), if any.
#[tauri::command]
pub async fn current_recording(
    state: State<'_, RecorderState>,
) -> Result<Option<RecordingStarted>, String> {
    Ok(state.0.lock().await.as_ref().map(|a| RecordingStarted {
        meeting_id: a.meeting_id,
        started_at: a.started_at,
        sources: a.sources.clone(),
    }))
}

#[tauri::command]
pub async fn pause_recording(state: State<'_, RecorderState>) -> Result<(), String> {
    let current = state.0.lock().await;
    current.as_ref().ok_or("no recording")?.recording.pause();
    Ok(())
}

#[tauri::command]
pub async fn resume_recording(state: State<'_, RecorderState>) -> Result<(), String> {
    let current = state.0.lock().await;
    current.as_ref().ok_or("no recording")?.recording.resume();
    Ok(())
}

/// Mute or unmute one source; a muted source records silence so tracks stay aligned.
#[tauri::command]
pub async fn set_source_muted(
    state: State<'_, RecorderState>,
    source_id: String,
    muted: bool,
) -> Result<(), String> {
    let current = state.0.lock().await;
    current
        .as_ref()
        .ok_or("no recording")?
        .recording
        .set_muted(&source_id, muted);
    Ok(())
}

/// Stop the recording, close the FLAC files, store the tracks and finish the meeting.
#[tauri::command]
pub async fn stop_recording(
    state: State<'_, RecorderState>,
    db: State<'_, Db>,
) -> Result<RecordingStopped, String> {
    let active = state.0.lock().await.take().ok_or("no recording")?;
    let meeting_id = active.meeting_id;
    let tracks = tauri::async_runtime::spawn_blocking(move || active.recording.stop())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    for (t, source) in tracks.iter().zip(&active.sources) {
        let kind = match t.kind {
            SourceKind::Mic => "mic",
            SourceKind::Loopback => "loopback",
        };
        let track = NewTrack {
            source_kind: kind,
            source_id: &t.source_id,
            device_name: &source.name,
            path: &t.file.display().to_string(),
            seconds: t.seconds,
        };
        db.add_track(meeting_id, &track)
            .await
            .map_err(|e| e.to_string())?;
    }
    let duration_ms = tracks.iter().map(|t| t.seconds).fold(0.0, f64::max) * 1000.0;
    // Playback mixdown; a failure here must not lose the recording itself.
    let files: Vec<std::path::PathBuf> = tracks.iter().map(|t| t.file.clone()).collect();
    let mix_path = files
        .first()
        .and_then(|f| f.parent())
        .map(|dir| dir.join(MIX_FILE));
    let mix = match mix_path {
        Some(path) => {
            let target = path.clone();
            match tauri::async_runtime::spawn_blocking(move || audio::mixdown(&files, &target))
                .await
            {
                Ok(Ok(_)) => Some(path.display().to_string()),
                Ok(Err(e)) => {
                    eprintln!("mixdown failed for meeting {meeting_id}: {e}");
                    None
                }
                Err(e) => {
                    eprintln!("mixdown task failed for meeting {meeting_id}: {e}");
                    None
                }
            }
        }
        None => None,
    };
    db.finish_meeting(meeting_id, duration_ms.round() as i64, "done")
        .await
        .map_err(|e| e.to_string())?;
    Ok(RecordingStopped {
        meeting_id,
        tracks,
        mix,
    })
}

/// Rename a meeting (also while it is being recorded).
#[tauri::command]
pub async fn rename_meeting(db: State<'_, Db>, id: i64, title: String) -> Result<(), String> {
    db.rename_meeting(id, title.trim())
        .await
        .map_err(|e| e.to_string())
}

/// A meeting as listed in the Library, with its named speakers.
#[derive(Debug, Serialize)]
pub struct MeetingSummary {
    #[serde(flatten)]
    pub meeting: Meeting,
    pub speakers: Vec<Speaker>,
}

/// All meetings, newest first.
#[tauri::command]
pub async fn list_meetings(db: State<'_, Db>) -> Result<Vec<MeetingSummary>, String> {
    let meetings = db.list_meetings().await.map_err(|e| e.to_string())?;
    let mut speakers = db.meeting_speakers().await.map_err(|e| e.to_string())?;
    Ok(meetings
        .into_iter()
        .map(|meeting| {
            let mine = speakers
                .extract_if(.., |(m, _)| *m == meeting.id)
                .map(|(_, s)| s)
                .collect();
            MeetingSummary {
                meeting,
                speakers: mine,
            }
        })
        .collect())
}

/// A meeting with what's needed to play it back.
#[derive(Debug, Serialize)]
pub struct MeetingDetail {
    #[serde(flatten)]
    pub meeting: Meeting,
    /// URL of the playback mixdown on the local media server, if the mix exists.
    pub mix_url: Option<String>,
}

#[tauri::command]
pub async fn get_meeting(
    db: State<'_, Db>,
    media: State<'_, MediaServer>,
    id: i64,
) -> Result<Option<MeetingDetail>, String> {
    let Some(meeting) = db.get_meeting(id).await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let has_mix = std::path::Path::new(&meeting.dir).join(MIX_FILE).is_file();
    Ok(Some(MeetingDetail {
        mix_url: has_mix.then(|| media.mix_url(meeting.id)),
        meeting,
    }))
}

/// Delete a meeting: its audio folder (only if it lies inside the meetings folder) and its
/// database rows (tracks and segments by cascade). Returns false if it didn't exist.
#[tauri::command]
pub async fn delete_meeting(
    db: State<'_, Db>,
    paths: State<'_, AppPaths>,
    id: i64,
) -> Result<bool, String> {
    let Some(meeting) = db.get_meeting(id).await.map_err(|e| e.to_string())? else {
        return Ok(false);
    };
    if let Some(dir) = paths.contained_meeting_dir(std::path::Path::new(&meeting.dir)) {
        std::fs::remove_dir_all(&dir)
            .map_err(|e| format!("could not delete {}: {e}", dir.display()))?;
    }
    db.delete_meeting(id).await.map_err(|e| e.to_string())
}

/// Engine and model state (Settings > Transcription, VRT-14).
#[tauri::command]
pub fn engine_status(engine: State<'_, EngineManager>) -> EngineStatus {
    engine.status()
}

/// Download the engine and the model in the background; progress comes as `engine:status`.
#[tauri::command]
pub fn engine_download(engine: State<'_, EngineManager>) -> Result<(), String> {
    engine.start_download()
}

/// Stop a running download; the part already downloaded is kept for the next attempt.
#[tauri::command]
pub fn engine_cancel_download(engine: State<'_, EngineManager>) {
    engine.cancel_download();
}

/// Start the engine helper (loads the model) and report its state.
#[tauri::command]
pub async fn engine_start(engine: State<'_, EngineManager>) -> Result<EngineStatus, String> {
    engine.start().await?;
    // A round trip through the helper's request loop, not just a loaded model.
    engine.request("ping", None).await?;
    Ok(engine.status())
}

/// The model licences and attributions (NOTICE file), once downloaded.
#[tauri::command]
pub fn engine_notice(engine: State<'_, EngineManager>) -> Option<String> {
    engine.notice()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(id: &str, kind: SourceKind, is_default: bool) -> SourceInfo {
        SourceInfo {
            id: id.into(),
            name: id.into(),
            kind,
            is_default,
        }
    }

    #[test]
    fn configured_sources_win_when_available() {
        let available = [
            src("mic-a", SourceKind::Mic, true),
            src("mic-b", SourceKind::Mic, false),
            src("out", SourceKind::Loopback, true),
        ];
        let configured = vec!["mic-b".to_string(), "gone".to_string()];
        let ids: Vec<_> = resolve_sources(&available, Some(&configured))
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(ids, ["mic-b"]);
    }

    #[test]
    fn defaults_are_used_without_a_usable_configuration() {
        let available = [
            src("mic-a", SourceKind::Mic, false),
            src("mic-default", SourceKind::Mic, true),
            src("out-a", SourceKind::Loopback, false),
        ];
        let ids = |configured: Option<&[String]>| -> Vec<String> {
            resolve_sources(&available, configured)
                .into_iter()
                .map(|s| s.id)
                .collect()
        };
        assert_eq!(
            ids(None),
            ["mic-default", "out-a"],
            "default mic, else first of kind"
        );
        assert_eq!(ids(Some(&["gone".to_string()])), ["mic-default", "out-a"]);
        assert!(resolve_sources(&[], None).is_empty());
    }

    #[test]
    fn source_settings_split_mic_and_system() {
        let available = vec![
            src("mic-default", SourceKind::Mic, true),
            src("mic-b", SourceKind::Mic, false),
            src("out", SourceKind::Loopback, true),
        ];
        let s = source_settings(available.clone(), None);
        assert_eq!(
            (s.mic.as_deref(), s.system.as_deref()),
            (Some("mic-default"), Some("out"))
        );
        let mic_only = vec!["mic-b".to_string()];
        let s = source_settings(available, Some(&mic_only));
        assert_eq!(
            (s.mic.as_deref(), s.system.as_deref()),
            (Some("mic-b"), None),
            "system off"
        );
    }

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
