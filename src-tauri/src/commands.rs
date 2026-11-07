//! Tauri commands exposed to the UI. Keep in sync with `src/lib/ipc.ts`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::audio::{self, Gap, Monitor, Recording, SourceInfo, SourceKind, TrackSummary};
use crate::db::Pass;
use crate::db::{Db, Meeting, NewSegment, NewTrack, Person, Speaker};
use crate::engine::transcript::Word;
use crate::engine::{EngineManager, EngineStatus};
use crate::jobs::{JobView, Jobs};
use crate::live::{self, Caption, SharedHistory, TrackRef};
use crate::media_server::{MIX_FILE, MediaServer};
use crate::passes::{self, Labelled};
use crate::paths::AppPaths;
use crate::power::SleepBlock;
use crate::rolling::{self, LabelVoice, PassOptions, PassResult, PassTrack, Rolling};
use crate::speakers;

/// Basic app information, shown in Settings > About.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub os: String,
    pub arch: String,
    /// The OS asks consent to capture audio and `open_privacy_settings` can show where (VRT-12).
    pub privacy_settings: bool,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "Manent".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        privacy_settings: audio::privacy_settings_url(SourceKind::Loopback).is_some(),
    }
}

/// Open the OS settings where the user allows capturing this kind of source (macOS, VRT-12).
#[tauri::command]
pub fn open_privacy_settings(kind: SourceKind) -> Result<(), String> {
    let url = audio::privacy_settings_url(kind).ok_or("Not available on this system.")?;
    crate::open_url::open(url);
    Ok(())
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
    /// Database rows of the tracks, in source order (created at start, VRT-16).
    track_ids: Vec<i64>,
    captions: bool,
    /// In a room (several people on the mic) or a call; switchable while recording (VRT-47).
    in_room: Arc<AtomicBool>,
    /// Live transcription: caption workers (VRT-16) and rolling passes (VRT-17).
    workers: Workers,
    rolling: Option<Rolling>,
    /// Keeps the computer from idle sleep until the recording stops (VRT-36).
    _awake: SleepBlock,
}

/// The caption workers of a recording and the track audio they keep for the passes.
type Workers = Arc<Mutex<Vec<(PassTrack, std::thread::JoinHandle<()>)>>>;

/// The recording in progress, if any.
#[derive(Default)]
pub struct RecorderState(tokio::sync::Mutex<Option<ActiveRecording>>);

impl RecorderState {
    /// Whether a recording runs, without waiting (false while a start or stop is under way).
    pub fn is_recording(&self) -> bool {
        self.0.try_lock().is_ok_and(|r| r.is_some())
    }

    /// Whether a recording runs, after a start or stop under way has finished.
    pub async fn is_recording_now(&self) -> bool {
        self.0.lock().await.is_some()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordingStarted {
    pub meeting_id: i64,
    /// Unix epoch milliseconds.
    pub started_at: i64,
    pub sources: Vec<SourceInfo>,
    /// Live captions are on (the engine and model are downloaded).
    pub captions: bool,
    /// In a room: everyone there is on the mic; otherwise a call, the mic is the user (VRT-47).
    pub in_room: bool,
}

/// A transcript segment for the UI: a live caption (no label) or a pass's speaker block.
#[derive(Debug, Clone, Serialize)]
pub struct TranscriptSegment {
    pub meeting_id: i64,
    pub track_id: i64,
    pub kind: SourceKind,
    /// "me", "1", "2", ... (None for live captions).
    pub label: Option<String>,
    /// "caption", "rolling" or "final".
    pub pass: String,
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,
    pub words: Vec<Word>,
}

/// A rolling pass replaced the segments that start in `[from_ms, to_ms)` (event
/// `transcript:pass`); the final pass sends the whole meeting (`transcript:final`).
#[derive(Debug, Clone, Serialize)]
pub struct PassEvent {
    pub meeting_id: i64,
    pub from_ms: u32,
    pub to_ms: u32,
    pub segments: Vec<TranscriptSegment>,
}

/// A caption could not be made (event `transcript:error`); the recording goes on.
#[derive(Debug, Clone, Serialize)]
pub struct CaptionError {
    pub meeting_id: i64,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct RecordingStopped {
    pub meeting_id: i64,
    pub tracks: Vec<TrackSummary>,
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

pub(crate) fn now_ms() -> Result<i64, String> {
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
    engine: State<'_, EngineManager>,
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
    // A call or a room, as the last recording was (VRT-47).
    let in_room = db
        .get_setting::<bool>(IN_ROOM_SETTING)
        .await
        .ok()
        .flatten()
        .unwrap_or(false);
    db.set_in_room(meeting_id, in_room)
        .await
        .map_err(|e| e.to_string())?;
    let in_room = Arc::new(AtomicBool::new(in_room));
    // Track rows first, so live captions can refer to them while recording.
    let mut track_ids = Vec::new();
    for (source, name) in sources.iter().zip(audio::track_file_names(&sources)) {
        let track = NewTrack {
            source_kind: kind_str(source.kind),
            source_id: &source.id,
            device_name: &source.name,
            path: &dir.join(name).display().to_string(),
            seconds: 0.0,
        };
        match db.add_track(meeting_id, &track).await {
            Ok(id) => track_ids.push(id),
            Err(e) => {
                let _ = db.delete_meeting(meeting_id).await;
                return Err(e.to_string());
            }
        }
    }
    let status = engine.status();
    let captions = status.engine_installed && status.model_installed;
    let events = app.clone();
    let workers: Workers = Arc::default();
    let on_track = {
        let (app, db, engine, ids, workers) = (
            app.clone(),
            db.inner().clone(),
            engine.inner().clone(),
            track_ids.clone(),
            workers.clone(),
        );
        move |i: usize, source: &SourceInfo, rate: u32, channels: u16| {
            if !captions {
                return None;
            }
            let track = TrackRef {
                meeting_id,
                track_id: ids[i],
                source_id: source.id.clone(),
                kind: source.kind,
            };
            let (app, err_app, db) = (app.clone(), app.clone(), db.clone());
            let on_caption = move |c: Caption| {
                let segment = NewSegment {
                    track_id: Some(c.track_id),
                    start_ms: c.start_ms.into(),
                    end_ms: c.end_ms.into(),
                    text: c.text.clone(),
                    words_json: serde_json::to_string(&c.words).ok(),
                    ..Default::default()
                };
                let saved = tauri::async_runtime::block_on(db.insert_segments(
                    c.meeting_id,
                    Pass::Caption,
                    &[segment],
                ));
                if let Err(e) = saved {
                    eprintln!("saving a caption failed: {e}");
                }
                let _ = app.emit("transcript:caption", c);
            };
            let on_error = move |message: String| {
                eprintln!("caption failed: {message}");
                let _ = err_app.emit(
                    "transcript:error",
                    CaptionError {
                        meeting_id,
                        message,
                    },
                );
            };
            let history = SharedHistory::default();
            let spawned = live::spawn(
                track.clone(),
                rate,
                channels,
                history.clone(),
                engine.clone(),
                on_caption,
                on_error,
            );
            match spawned {
                Ok((feed, worker)) => {
                    let pass_track = PassTrack { track, history };
                    lock(&workers).push((pass_track, worker));
                    Some(feed)
                }
                Err(e) => {
                    eprintln!("starting live captions failed: {e}");
                    None
                }
            }
        }
    };
    let opening = std::time::Instant::now();
    let recording = match Recording::start(
        &dir,
        sources.clone(),
        move |level| {
            let _ = events.emit("audio:level", level);
        },
        on_track,
    ) {
        Ok(r) => r,
        Err(e) => {
            let _ = db.delete_meeting(meeting_id).await;
            let _ = std::fs::remove_dir_all(&dir);
            return Err(e.to_string());
        }
    };
    // Track time 0 is when capture began, after the devices opened (this can take a moment
    // with PipeWire); the UI timer and caption times use this, not the time of the click.
    let started_at = started_at + opening.elapsed().as_millis() as i64;
    eprintln!(
        "recording {meeting_id}: capture started {} ms after the request",
        opening.elapsed().as_millis()
    );
    let mut passes = None;
    if captions {
        // Load the model now, so the first chunk doesn't wait for it.
        let starting = engine.inner().clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = starting.start().await {
                eprintln!("engine start for live captions failed: {e}");
            }
        });
        let tracks: Vec<PassTrack> = lock(&workers)
            .iter()
            .map(|(t, _)| PassTrack {
                track: t.track.clone(),
                history: t.history.clone(),
            })
            .collect();
        let seconds: Option<u64> = db
            .get_setting(ROLLING_INTERVAL_SETTING)
            .await
            .map_err(|e| e.to_string())?;
        let interval = seconds.map_or(rolling::DEFAULT_INTERVAL, std::time::Duration::from_secs);
        passes = start_rolling(
            &app,
            db.inner(),
            engine.inner(),
            &paths,
            meeting_id,
            tracks,
            PassOptions {
                interval,
                in_room: in_room.clone(),
            },
        );
    }
    let awake = SleepBlock::acquire("Recording a meeting");
    eprintln!(
        "recording {meeting_id}: keeping the computer awake: {}",
        if awake.is_held() { "yes" } else { "no" }
    );
    *current = Some(ActiveRecording {
        recording,
        meeting_id,
        started_at,
        sources: sources.clone(),
        track_ids,
        captions,
        in_room: in_room.clone(),
        workers,
        rolling: passes,
        _awake: awake,
    });
    let started = RecordingStarted {
        meeting_id,
        started_at,
        sources,
        captions,
        in_room: in_room.load(Ordering::SeqCst),
    };
    // The tray and the window follow, whoever started it (VRT-22).
    crate::tray::set_recording(&app, true);
    let _ = app.emit("recording:started", &started);
    Ok(started)
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
        captions: a.captions,
        in_room: a.in_room.load(Ordering::SeqCst),
    }))
}

/// Settings key for the last choice between a call and a room (VRT-47).
pub const IN_ROOM_SETTING: &str = "recording.in_room";

/// The choice between a call and a room the next recording starts with (VRT-54).
#[tauri::command]
pub async fn get_in_room(db: State<'_, Db>) -> Result<bool, String> {
    Ok(db
        .get_setting::<bool>(IN_ROOM_SETTING)
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or(false))
}

/// Choose between a call (the mic is the user) and a room (everyone there is on the mic). A
/// running recording follows from its next rolling pass and in its final pass; the next
/// recording starts the same way.
#[tauri::command]
pub async fn set_recording_in_room(
    state: State<'_, RecorderState>,
    db: State<'_, Db>,
    in_room: bool,
) -> Result<(), String> {
    let current = state.0.lock().await;
    if let Some(active) = current.as_ref() {
        active.in_room.store(in_room, Ordering::SeqCst);
        db.set_in_room(active.meeting_id, in_room)
            .await
            .map_err(|e| e.to_string())?;
    }
    db.set_setting(IN_ROOM_SETTING, &in_room)
        .await
        .map_err(|e| e.to_string())
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
    app: AppHandle,
    state: State<'_, RecorderState>,
    db: State<'_, Db>,
    jobs: State<'_, Jobs>,
) -> Result<RecordingStopped, String> {
    let ActiveRecording {
        recording,
        meeting_id,
        track_ids,
        workers,
        rolling,
        ..
    } = state.0.lock().await.take().ok_or("no recording")?;
    let audio::Stopped { tracks, gaps } =
        tauri::async_runtime::spawn_blocking(move || recording.stop())
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
    let gaps: Vec<(i64, i64)> = gaps.iter().map(|g| (g.at_ms, g.missing_ms)).collect();
    db.add_gaps(meeting_id, &gaps)
        .await
        .map_err(|e| e.to_string())?;
    // Live transcription winds down: the caption workers finish their last chunk, then the
    // rolling passes stop, so nothing writes captions after the final pass replaced them.
    let pass_tracks: Vec<TrackRef> = tauri::async_runtime::spawn_blocking(move || {
        let list = std::mem::take(&mut *lock(&workers));
        if let Some(r) = rolling {
            r.stop();
        }
        list.into_iter()
            .map(|(t, worker)| {
                let _ = worker.join();
                t.track
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())?;
    for (t, id) in tracks.iter().zip(&track_ids) {
        db.set_track_seconds(*id, t.seconds)
            .await
            .map_err(|e| e.to_string())?;
    }
    let duration_ms =
        (tracks.iter().map(|t| t.seconds).fold(0.0, f64::max) * 1000.0).round() as i64;
    // The playback file, then (with transcription on) the final pass run as a job
    // ("processing"), so Stop returns at once, also after a long meeting (VRT-45).
    db.finish_meeting(meeting_id, duration_ms, "processing")
        .await
        .map_err(|e| e.to_string())?;
    let title = db
        .get_meeting(meeting_id)
        .await
        .ok()
        .flatten()
        .map(|m| m.title)
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "Untitled meeting".into());
    jobs.finish_recording(meeting_id, title, !pass_tracks.is_empty());
    crate::tray::set_recording(&app, false);
    let _ = app.emit("recording:stopped", meeting_id);
    Ok(RecordingStopped { meeting_id, tracks })
}

/// A mark was added, changed or deleted (event `meeting:marks`, VRT-52).
#[derive(Debug, Clone, Serialize)]
pub struct MarksChanged {
    pub meeting_id: i64,
    /// The mark just added (the Live screen shows it at once).
    pub added: Option<crate::db::Mark>,
}

/// Mark the current moment of the running recording.
async fn mark_now(app: &AppHandle) -> Result<crate::db::Mark, String> {
    let (meeting_id, at_ms) = {
        let state = app.state::<RecorderState>();
        let current = state.0.lock().await;
        let active = current
            .as_ref()
            .ok_or("Not recording: start a recording to mark")?;
        (active.meeting_id, active.recording.position_ms())
    };
    let db = app.state::<Db>();
    let mark = db
        .add_mark(meeting_id, at_ms as i64, now_ms()?)
        .await
        .map_err(|e| e.to_string())?;
    let _ = app.emit(
        "meeting:marks",
        MarksChanged {
            meeting_id,
            added: Some(mark.clone()),
        },
    );
    crate::transcript_md::spawn_write(db.inner().clone(), vec![meeting_id]);
    Ok(mark)
}

/// Mark this moment of the recording (the Live screen's Mark button).
#[tauri::command]
pub async fn mark_moment(app: AppHandle) -> Result<crate::db::Mark, String> {
    mark_now(&app).await
}

/// The mark-moment shortcut (VRT-40): mark, or say why not in the log.
pub fn mark_from_shortcut(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        match mark_now(&app).await {
            Ok(m) => eprintln!("marked meeting {} at {} ms", m.meeting_id, m.at_ms),
            Err(e) => eprintln!("mark: {e}"),
        }
    });
}

#[tauri::command]
pub async fn list_marks(
    db: State<'_, Db>,
    meeting_id: i64,
) -> Result<Vec<crate::db::Mark>, String> {
    db.list_marks(meeting_id).await.map_err(|e| e.to_string())
}

fn marks_changed(app: &AppHandle, db: &Db, meeting_id: i64) {
    let _ = app.emit(
        "meeting:marks",
        MarksChanged {
            meeting_id,
            added: None,
        },
    );
    crate::transcript_md::spawn_write(db.clone(), vec![meeting_id]);
}

/// Give a mark a label (empty removes it).
#[tauri::command]
pub async fn set_mark_label(
    app: AppHandle,
    db: State<'_, Db>,
    id: i64,
    label: String,
) -> Result<(), String> {
    if let Some(m) = db
        .set_mark_label(id, &label)
        .await
        .map_err(|e| e.to_string())?
    {
        marks_changed(&app, db.inner(), m.meeting_id);
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_mark(app: AppHandle, db: State<'_, Db>, id: i64) -> Result<(), String> {
    if let Some(meeting_id) = db.delete_mark(id).await.map_err(|e| e.to_string())? {
        marks_changed(&app, db.inner(), meeting_id);
    }
    Ok(())
}

/// Rename a meeting (also while it is being recorded).
#[tauri::command]
pub async fn rename_meeting(db: State<'_, Db>, id: i64, title: String) -> Result<(), String> {
    db.rename_meeting(id, title.trim())
        .await
        .map_err(|e| e.to_string())?;
    crate::transcript_md::spawn_write(db.inner().clone(), vec![id]);
    Ok(())
}

/// A meeting as listed in the Library, with its named speakers.
#[derive(Debug, Serialize)]
pub struct MeetingSummary {
    #[serde(flatten)]
    pub meeting: Meeting,
    pub speakers: Vec<Speaker>,
    /// Has a folder on the Nextcloud server (VRT-39).
    pub synced: bool,
}

/// All meetings, newest first.
#[tauri::command]
pub async fn list_meetings(db: State<'_, Db>) -> Result<Vec<MeetingSummary>, String> {
    let meetings = db.list_meetings().await.map_err(|e| e.to_string())?;
    let mut speakers = db.meeting_speakers().await.map_err(|e| e.to_string())?;
    let synced: Vec<i64> = db
        .sync_meetings()
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|s| !s.gone)
        .map(|s| s.meeting_id)
        .collect();
    Ok(meetings
        .into_iter()
        .map(|meeting| {
            let mine = speakers
                .extract_if(.., |(m, _)| *m == meeting.id)
                .map(|(_, s)| s)
                .collect();
            MeetingSummary {
                synced: synced.contains(&meeting.id),
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
    sync: State<'_, crate::sync::Sync>,
    id: i64,
    also_remote: Option<bool>,
) -> Result<bool, String> {
    let Some(meeting) = db.get_meeting(id).await.map_err(|e| e.to_string())? else {
        return Ok(false);
    };
    // Its folder on Nextcloud too, when the user chose so (VRT-39); first, so a failure
    // leaves everything as it was.
    if also_remote == Some(true) {
        sync.delete_remote(id)
            .await
            .map_err(|e| format!("Could not remove it from Nextcloud: {e}"))?;
    }
    if let Some(dir) = paths.contained_meeting_dir(std::path::Path::new(&meeting.dir)) {
        std::fs::remove_dir_all(&dir)
            .map_err(|e| format!("could not delete {}: {e}", dir.display()))?;
    }
    db.delete_meeting(id).await.map_err(|e| e.to_string())
}

fn kind_str(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::Mic => "mic",
        SourceKind::Loopback => "loopback",
    }
}

/// The transcript of a meeting so far (captions and pass segments in time order), e.g.
/// when the Live screen is opened again.
#[tauri::command]
pub async fn live_transcript(
    db: State<'_, Db>,
    meeting_id: i64,
) -> Result<Vec<TranscriptSegment>, String> {
    let tracks = db
        .list_tracks(meeting_id)
        .await
        .map_err(|e| e.to_string())?;
    let segments = db
        .list_segments(meeting_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(segments
        .into_iter()
        .filter_map(|s| {
            let track = tracks.iter().find(|t| Some(t.id) == s.track_id)?;
            Some(TranscriptSegment {
                meeting_id,
                track_id: track.id,
                kind: if track.source_kind == "mic" {
                    SourceKind::Mic
                } else {
                    SourceKind::Loopback
                },
                label: s.speaker_label,
                pass: s.pass,
                start_ms: s.start_ms as u32,
                end_ms: s.end_ms as u32,
                text: s.text,
                words: s
                    .words_json
                    .as_deref()
                    .and_then(|w| serde_json::from_str(w).ok())
                    .unwrap_or_default(),
            })
        })
        .collect())
}

/// Settings key for the name shown for the mic track's speaker ("Me" by default).
pub const ME_NAME_SETTING: &str = "speaker.me_name";

/// The name shown for the local user's speech (the mic track).
#[tauri::command]
pub async fn get_me_name(db: State<'_, Db>) -> Result<String, String> {
    let name: Option<String> = db
        .get_setting(ME_NAME_SETTING)
        .await
        .map_err(|e| e.to_string())?;
    Ok(name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "Me".into()))
}

#[tauri::command]
pub async fn set_me_name(db: State<'_, Db>, name: String) -> Result<(), String> {
    db.set_setting(ME_NAME_SETTING, &name.trim())
        .await
        .map_err(|e| e.to_string())?;
    crate::transcript_md::spawn_write_all(db.inner().clone());
    Ok(())
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) fn new_segment(track_id: i64, b: &Labelled) -> NewSegment {
    NewSegment {
        track_id: Some(track_id),
        speaker_label: Some(b.label.clone()),
        start_ms: b.start_ms.into(),
        end_ms: b.end_ms.into(),
        text: b.text.clone(),
        words_json: serde_json::to_string(&b.words).ok(),
        ..Default::default()
    }
}

pub(crate) fn segment_view(
    meeting_id: i64,
    track_id: i64,
    kind: SourceKind,
    pass: &str,
    b: Labelled,
) -> TranscriptSegment {
    TranscriptSegment {
        meeting_id,
        track_id,
        kind,
        label: Some(b.label),
        pass: pass.into(),
        start_ms: b.start_ms,
        end_ms: b.end_ms,
        text: b.text,
        words: b.words,
    }
}

/// Rolling passes during a recording (VRT-17): each result replaces the captions and earlier
/// rolling segments of its stretch, in the database and on the Live screen.
fn start_rolling(
    app: &AppHandle,
    db: &Db,
    engine: &EngineManager,
    paths: &AppPaths,
    meeting_id: i64,
    tracks: Vec<PassTrack>,
    options: PassOptions,
) -> Option<Rolling> {
    let kinds: HashMap<i64, SourceKind> = tracks
        .iter()
        .map(|t| (t.track.track_id, t.track.kind))
        .collect();
    let (app, err_app, db) = (app.clone(), app.clone(), db.clone());
    let on_pass = move |r: PassResult| {
        let rows: Vec<NewSegment> = r.blocks.iter().map(|(id, b)| new_segment(*id, b)).collect();
        let saved = tauri::async_runtime::block_on(db.replace_segments(
            meeting_id,
            &[Pass::Caption, Pass::Rolling],
            r.from_ms.into(),
            r.to_ms.into(),
            Pass::Rolling,
            &rows,
        ));
        if let Err(e) = saved {
            eprintln!("saving a rolling pass failed: {e}");
        }
        if let Err(e) = tauri::async_runtime::block_on(store_voices(&db, meeting_id, &r.voices)) {
            eprintln!("matching speakers failed: {e}");
        }
        let segments = r
            .blocks
            .into_iter()
            .map(|(id, b)| segment_view(meeting_id, id, kinds[&id], "rolling", b))
            .collect();
        let _ = app.emit(
            "transcript:pass",
            PassEvent {
                meeting_id,
                from_ms: r.from_ms,
                to_ms: r.to_ms,
                segments,
            },
        );
        let _ = app.emit(
            "speakers:changed",
            SpeakersChanged {
                meeting_id: Some(meeting_id),
            },
        );
    };
    let on_error = move |message: String| {
        eprintln!("rolling pass failed: {message}");
        let _ = err_app.emit(
            "transcript:error",
            CaptionError {
                meeting_id,
                message,
            },
        );
    };
    eprintln!(
        "meeting {meeting_id}: rolling passes every {} s",
        options.interval.as_secs()
    );
    match rolling::spawn(
        tracks,
        engine.clone(),
        paths.tmp_dir(),
        options,
        on_pass,
        on_error,
    ) {
        Ok(r) => Some(r),
        Err(e) => {
            eprintln!("starting the rolling passes failed: {e}");
            None
        }
    }
}

/// Who the speakers are changed (event `speakers:changed`); None = people in general.
#[derive(Debug, Clone, Serialize)]
pub struct SpeakersChanged {
    pub meeting_id: Option<i64>,
}

/// Store a pass's voices and match them against the registry (VRT-18).
async fn store_voices(db: &Db, meeting_id: i64, voices: &[LabelVoice]) -> Result<(), String> {
    for v in voices {
        db.upsert_meeting_voice(
            meeting_id,
            &v.label,
            v.embedding.as_deref(),
            &v.encoder_id,
            v.clean_seconds.into(),
        )
        .await
        .map_err(|e| e.to_string())?;
    }
    speakers::identify(db, meeting_id).await
}

/// Stretches of the meeting the user named before the final pass: (track, start, end, person).
pub(crate) async fn named_stretches(db: &Db, meeting_id: i64) -> Vec<(i64, i64, i64, i64)> {
    let (Ok(voices), Ok(segments)) = (
        db.meeting_voices(meeting_id).await,
        db.list_segments(meeting_id).await,
    ) else {
        return Vec::new();
    };
    let confirmed: HashMap<String, i64> = voices
        .into_iter()
        .filter(|v| v.confirmed)
        .filter_map(|v| Some((v.label, v.speaker_id?)))
        .collect();
    segments
        .into_iter()
        .filter_map(|s| {
            let person = confirmed.get(s.speaker_label.as_deref()?)?;
            Some((s.track_id?, s.start_ms, s.end_ms, *person))
        })
        .collect()
}

/// After the final pass: the new labels take the names the user gave during the recording
/// (by overlap with the named stretches), then the rest is matched against the registry.
pub(crate) async fn relabel_final(
    db: &Db,
    meeting_id: i64,
    blocks: &[(i64, Labelled)],
    voices: &[LabelVoice],
    named: &[(i64, i64, i64, i64)],
) -> Result<(), String> {
    db.clear_meeting_voices(meeting_id)
        .await
        .map_err(|e| e.to_string())?;
    for v in voices {
        let mut total = 0i64;
        let mut overlap: HashMap<i64, i64> = HashMap::new();
        for (track, b) in blocks.iter().filter(|(_, b)| b.label == v.label) {
            let (start, end) = (i64::from(b.start_ms), i64::from(b.end_ms));
            total += end - start;
            for (_, s, e, person) in named.iter().filter(|n| n.0 == *track) {
                let o = end.min(*e) - start.max(*s);
                if o > 0 {
                    *overlap.entry(*person).or_default() += o;
                }
            }
        }
        db.upsert_meeting_voice(
            meeting_id,
            &v.label,
            v.embedding.as_deref(),
            &v.encoder_id,
            v.clean_seconds.into(),
        )
        .await
        .map_err(|e| e.to_string())?;
        if let Some((person, o)) = overlap.into_iter().max_by_key(|(_, o)| *o)
            && o * 2 >= total
        {
            db.set_voice_identity(meeting_id, &v.label, Some(person), None, None, true)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    speakers::identify(db, meeting_id).await
}

/// A meeting's speaker label as the UI shows it: who it is or who it might be.
#[derive(Debug, Clone, Serialize)]
pub struct VoiceView {
    pub label: String,
    pub speaker: Option<Speaker>,
    pub suggestion: Option<Speaker>,
    pub distance: Option<f64>,
    pub clean_seconds: f64,
    pub confirmed: bool,
}

/// Where a meeting's recording lost audio while the computer slept (VRT-36).
#[tauri::command]
pub async fn meeting_gaps(db: State<'_, Db>, meeting_id: i64) -> Result<Vec<Gap>, String> {
    Ok(db
        .list_gaps(meeting_id)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(at_ms, missing_ms)| Gap { at_ms, missing_ms })
        .collect())
}

#[tauri::command]
pub async fn meeting_voices(db: State<'_, Db>, meeting_id: i64) -> Result<Vec<VoiceView>, String> {
    let people: HashMap<i64, Speaker> = db
        .list_speakers()
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|s| (s.id, s))
        .collect();
    Ok(db
        .meeting_voices(meeting_id)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|v| VoiceView {
            speaker: v.speaker_id.and_then(|id| people.get(&id).cloned()),
            suggestion: v.suggested_id.and_then(|id| people.get(&id).cloned()),
            label: v.label,
            distance: v.distance,
            clean_seconds: v.clean_seconds,
            confirmed: v.confirmed,
        })
        .collect())
}

/// Name a meeting's speaker (doc-2 §1.4): an existing person (same name) or a new one. The
/// voice becomes one of that person's samples, and every segment of the label is renamed.
#[tauri::command]
pub async fn name_speaker(
    app: AppHandle,
    db: State<'_, Db>,
    meeting_id: i64,
    label: String,
    name: String,
) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("a name is needed".into());
    }
    let err = |e: crate::db::DbError| e.to_string();
    let person = match db.speaker_by_name(name).await.map_err(err)? {
        Some(p) => p.id,
        None => {
            let count = db.list_speakers().await.map_err(err)?.len() as i64;
            db.create_speaker(name, count % 8 + 1).await.map_err(err)?
        }
    };
    assign_person(&db, meeting_id, &label, person).await?;
    let _ = app.emit(
        "speakers:changed",
        SpeakersChanged {
            meeting_id: Some(meeting_id),
        },
    );
    crate::transcript_md::spawn_write(db.inner().clone(), vec![meeting_id]);
    Ok(())
}

/// Accept the suggested name of a meeting's speaker.
#[tauri::command]
pub async fn confirm_suggestion(
    app: AppHandle,
    db: State<'_, Db>,
    meeting_id: i64,
    label: String,
) -> Result<(), String> {
    let voices = db
        .meeting_voices(meeting_id)
        .await
        .map_err(|e| e.to_string())?;
    let person = voices
        .iter()
        .find(|v| v.label == label)
        .and_then(|v| v.suggested_id)
        .ok_or("no suggestion for this speaker")?;
    assign_person(&db, meeting_id, &label, person).await?;
    let _ = app.emit(
        "speakers:changed",
        SpeakersChanged {
            meeting_id: Some(meeting_id),
        },
    );
    crate::transcript_md::spawn_write(db.inner().clone(), vec![meeting_id]);
    Ok(())
}

async fn assign_person(db: &Db, meeting_id: i64, label: &str, person: i64) -> Result<(), String> {
    let err = |e: crate::db::DbError| e.to_string();
    db.set_voice_identity(meeting_id, label, Some(person), None, None, true)
        .await
        .map_err(err)?;
    let voices = db.meeting_voices(meeting_id).await.map_err(err)?;
    if let Some(v) = voices.iter().find(|v| v.label == label)
        && let (Some(embedding), Some(encoder)) = (&v.embedding, &v.encoder_id)
        && v.clean_seconds >= speakers::MIN_CLEAN_SECONDS
    {
        db.add_voice_sample(
            person,
            embedding,
            encoder,
            v.clean_seconds,
            Some(meeting_id),
            now_ms()?,
        )
        .await
        .map_err(err)?;
    }
    // The new sample can name other speakers too.
    speakers::identify(db, meeting_id).await
}

/// Two speakers of a meeting are one person: `from` joins `into`.
#[tauri::command]
pub async fn merge_labels(
    app: AppHandle,
    db: State<'_, Db>,
    meeting_id: i64,
    from: String,
    into: String,
) -> Result<(), String> {
    if from == into || from == passes::ME || into == passes::ME {
        return Err("pick another speaker".into());
    }
    db.merge_labels(meeting_id, &from, &into)
        .await
        .map_err(|e| e.to_string())?;
    speakers::identify(&db, meeting_id).await?;
    let _ = app.emit(
        "speakers:changed",
        SpeakersChanged {
            meeting_id: Some(meeting_id),
        },
    );
    crate::transcript_md::spawn_write(db.inner().clone(), vec![meeting_id]);
    Ok(())
}

/// People on the Speakers screen.
#[tauri::command]
pub async fn list_people(db: State<'_, Db>) -> Result<Vec<Person>, String> {
    db.list_people().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rename_person(
    app: AppHandle,
    db: State<'_, Db>,
    id: i64,
    name: String,
) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("a name is needed".into());
    }
    db.rename_speaker(id, name)
        .await
        .map_err(|e| e.to_string())?;
    let _ = app.emit("speakers:changed", SpeakersChanged { meeting_id: None });
    crate::transcript_md::spawn_write_all(db.inner().clone());
    Ok(())
}

/// `merged` is the same person as `keep`: their voice samples and segments move to `keep`.
#[tauri::command]
pub async fn merge_people(
    app: AppHandle,
    db: State<'_, Db>,
    keep: i64,
    merged: i64,
) -> Result<(), String> {
    if keep == merged {
        return Err("pick another person".into());
    }
    db.merge_speakers(keep, merged)
        .await
        .map_err(|e| e.to_string())?;
    let _ = app.emit("speakers:changed", SpeakersChanged { meeting_id: None });
    crate::transcript_md::spawn_write_all(db.inner().clone());
    Ok(())
}

/// Delete a person and their voice data (biometric); their segments stay, unnamed.
#[tauri::command]
pub async fn delete_person(app: AppHandle, db: State<'_, Db>, id: i64) -> Result<(), String> {
    db.delete_speaker(id).await.map_err(|e| e.to_string())?;
    let _ = app.emit("speakers:changed", SpeakersChanged { meeting_id: None });
    crate::transcript_md::spawn_write_all(db.inner().clone());
    Ok(())
}

/// Import recordings made elsewhere (VRT-19): each file becomes a meeting with a transcript.
#[tauri::command]
pub fn import_audio(
    engine: State<'_, EngineManager>,
    jobs: State<'_, Jobs>,
    paths: Vec<String>,
) -> Result<Vec<u64>, String> {
    let status = engine.status();
    if !(status.engine_installed && status.model_installed) {
        return Err("Download the transcription engine in Settings before importing".into());
    }
    Ok(paths
        .into_iter()
        .map(|p| jobs.import(std::path::PathBuf::from(p)))
        .collect())
}

/// Transcribe a meeting again from its tracks, replacing its transcript (VRT-19).
#[tauri::command]
pub async fn retranscribe(
    db: State<'_, Db>,
    jobs: State<'_, Jobs>,
    meeting_id: i64,
) -> Result<u64, String> {
    if jobs.busy(meeting_id) {
        return Err("This meeting is already being transcribed".into());
    }
    let meeting = db
        .get_meeting(meeting_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("no such meeting")?;
    if meeting.status == "recording" {
        return Err("Stop the recording first".into());
    }
    let tracks = db
        .list_tracks(meeting_id)
        .await
        .map_err(|e| e.to_string())?;
    if tracks.is_empty()
        || tracks
            .iter()
            .any(|t| !std::path::Path::new(&t.path).is_file())
    {
        return Err("The original tracks of this meeting were not kept (Settings > Storage), so it can't be transcribed again".into());
    }
    let name = if meeting.title.is_empty() {
        "Untitled meeting".into()
    } else {
        meeting.title
    };
    Ok(jobs.transcribe(meeting_id, name))
}

#[tauri::command]
pub fn list_jobs(jobs: State<'_, Jobs>) -> Vec<JobView> {
    jobs.list()
}

#[tauri::command]
pub fn dismiss_job(jobs: State<'_, Jobs>, id: u64) {
    jobs.dismiss(id);
}

/// Bars of the meeting's waveform for the player (VRT-20); computed once from the tracks and
/// kept in the meeting folder.
#[tauri::command]
pub async fn meeting_waveform(
    db: State<'_, Db>,
    paths: State<'_, AppPaths>,
    id: i64,
) -> Result<Vec<f32>, String> {
    waveform_bars(&db, &paths, id).await
}

/// Bars in a meeting's waveform.
const WAVEFORM_BINS: usize = 600;
/// The cached waveform in the meeting folder.
const WAVEFORM_FILE: &str = "waveform.json";

/// A meeting's playback file and waveform from its tracks (VRT-45): mix.opus appears whole
/// (written aside, then renamed), so the player never opens a half-written file; the waveform
/// comes from the same decode.
pub(crate) fn make_playback(
    dir: &std::path::Path,
    files: &[std::path::PathBuf],
    progress: &mut dyn FnMut(f64),
) -> Result<audio::MixSummary, String> {
    let (part, mix) = (dir.join(format!("{MIX_FILE}.part")), dir.join(MIX_FILE));
    let summary = audio::mixdown_with_progress(files, &part, progress).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        e.to_string()
    })?;
    let bars = audio::waveform_from_power(&summary.frame_power, WAVEFORM_BINS);
    std::fs::write(
        dir.join(WAVEFORM_FILE),
        serde_json::to_vec(&bars).unwrap_or_default(),
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(&part, &mix).map_err(|e| e.to_string())?;
    Ok(summary)
}

pub(crate) async fn waveform_bars(db: &Db, paths: &AppPaths, id: i64) -> Result<Vec<f32>, String> {
    const BINS: usize = WAVEFORM_BINS;
    let meeting = db
        .get_meeting(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("no such meeting")?;
    let dir = paths
        .contained_meeting_dir(std::path::Path::new(&meeting.dir))
        .ok_or("the meeting folder is missing")?;
    let cache = dir.join(WAVEFORM_FILE);
    if let Some(bars) = std::fs::read(&cache)
        .ok()
        .and_then(|b| serde_json::from_slice::<Vec<f32>>(&b).ok())
        .filter(|b| b.len() == BINS)
    {
        return Ok(bars);
    }
    let files: Vec<std::path::PathBuf> = db
        .list_tracks(id)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|t| std::path::PathBuf::from(t.path))
        .filter(|p| p.is_file())
        .collect();
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let bars = tauri::async_runtime::spawn_blocking(move || {
        let refs: Vec<&std::path::Path> = files.iter().map(|p| p.as_path()).collect();
        audio::waveform(&refs, BINS)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    if meeting.status == "done" {
        let _ = std::fs::write(&cache, serde_json::to_vec(&bars).unwrap_or_default());
    }
    Ok(bars)
}

/// Write a meeting's transcript to `path` as Markdown, TXT, SRT or JSON (VRT-21). The path
/// comes from the save dialog; `date_label` is the meeting date as the UI shows it.
#[tauri::command]
pub async fn export_meeting(
    db: State<'_, Db>,
    id: i64,
    format: String,
    path: String,
    date_label: String,
) -> Result<(), String> {
    let format = crate::export::Format::parse(&format).ok_or("unknown export format")?;
    let (_, export, _) = crate::transcript_md::load(&db, id, date_label)
        .await?
        .ok_or("no such meeting")?;
    std::fs::write(&path, crate::export::render(&export, format))
        .map_err(|e| format!("can't write {path}: {e}"))
}

/// Full-text search over every transcript (VRT-20); diacritics and case are ignored.
#[tauri::command]
pub async fn search_transcripts(
    db: State<'_, Db>,
    query: String,
) -> Result<Vec<crate::db::SearchHit>, String> {
    db.search(&query, 50).await.map_err(|e| e.to_string())
}

/// Settings key: seconds between rolling passes while recording (VRT-27).
pub const ROLLING_INTERVAL_SETTING: &str = "transcription.rolling_interval_s";
/// Settings key: "all" (tracks and playback file) or "mix" (playback file only).
pub const KEEP_AUDIO_SETTING: &str = "storage.keep";
/// The rolling pass intervals offered (shorter costs CPU, longer delays speaker names).
pub const ROLLING_INTERVALS: [u64; 4] = [30, 45, 60, 90];

/// Settings > Transcription and Storage (VRT-27).
#[derive(Debug, Clone, Serialize)]
pub struct AppSettings {
    pub rolling_interval_s: u64,
    pub rolling_intervals: Vec<u64>,
    /// "all" or "mix".
    pub keep_audio: String,
    pub meetings_dir: String,
    pub default_meetings_dir: String,
}

async fn app_settings(db: &Db, paths: &AppPaths) -> Result<AppSettings, String> {
    let err = |e: crate::db::DbError| e.to_string();
    let interval: Option<u64> = db
        .get_setting(ROLLING_INTERVAL_SETTING)
        .await
        .map_err(err)?;
    let keep: Option<String> = db.get_setting(KEEP_AUDIO_SETTING).await.map_err(err)?;
    Ok(AppSettings {
        rolling_interval_s: interval.unwrap_or(rolling::DEFAULT_INTERVAL.as_secs()),
        rolling_intervals: ROLLING_INTERVALS.to_vec(),
        keep_audio: keep.unwrap_or_else(|| "all".into()),
        meetings_dir: paths.meetings_root().display().to_string(),
        default_meetings_dir: paths.data.join("meetings").display().to_string(),
    })
}

#[tauri::command]
pub async fn get_app_settings(
    db: State<'_, Db>,
    paths: State<'_, AppPaths>,
) -> Result<AppSettings, String> {
    app_settings(&db, &paths).await
}

/// How often speaker labels are updated while recording; applies to the next recording.
#[tauri::command]
pub async fn set_rolling_interval(db: State<'_, Db>, seconds: u64) -> Result<(), String> {
    if !ROLLING_INTERVALS.contains(&seconds) {
        return Err(format!("choose one of {ROLLING_INTERVALS:?} seconds"));
    }
    db.set_setting(ROLLING_INTERVAL_SETTING, &seconds)
        .await
        .map_err(|e| e.to_string())
}

/// Which audio new meetings keep after transcription: "all" or "mix" (playback file only).
#[tauri::command]
pub async fn set_keep_audio(db: State<'_, Db>, keep: String) -> Result<(), String> {
    if keep != "all" && keep != "mix" {
        return Err("keep must be \"all\" or \"mix\"".into());
    }
    db.set_setting(KEEP_AUDIO_SETTING, &keep)
        .await
        .map_err(|e| e.to_string())
}

/// Nothing may be recording or transcribing while files move or the engine changes.
async fn ensure_idle(recorder: &RecorderState, jobs: &Jobs) -> Result<(), String> {
    if recorder.0.lock().await.is_some() {
        return Err("Stop the recording first".into());
    }
    if jobs.any_running() {
        return Err("Wait until the running transcriptions are done".into());
    }
    Ok(())
}

/// Move every meeting to another folder (`to` = None: back to the default) and use it from
/// now on (VRT-27). The database follows the files.
#[tauri::command]
pub async fn move_meetings(
    db: State<'_, Db>,
    paths: State<'_, AppPaths>,
    recorder: State<'_, RecorderState>,
    jobs: State<'_, Jobs>,
    to: Option<String>,
) -> Result<AppSettings, String> {
    ensure_idle(&recorder, &jobs).await?;
    let err = |e: crate::db::DbError| e.to_string();
    let from = paths.meetings_root();
    let target = to
        .as_deref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| paths.data.join("meetings"));
    let (f, t) = (from.clone(), target.clone());
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::create_dir_all(&f)?;
        crate::paths::move_meeting_folders(&f, &t)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    point_meetings_to(&db, &target).await.map_err(err)?;
    let chosen = to.filter(|t| std::path::Path::new(t) != paths.data.join("meetings"));
    db.set_setting(crate::paths::MEETINGS_DIR_SETTING, &chosen)
        .await
        .map_err(err)?;
    paths.set_meetings_root(chosen.map(std::path::PathBuf::from));
    app_settings(&db, &paths).await
}

/// After a move: every meeting whose folder is now in `target` points there, with its tracks.
async fn point_meetings_to(db: &Db, target: &std::path::Path) -> crate::db::Result<()> {
    for m in db.list_meetings().await? {
        let Some(name) = std::path::Path::new(&m.dir).file_name() else {
            continue;
        };
        let dir = target.join(name);
        if !dir.is_dir() {
            continue;
        }
        db.set_meeting_dir(m.id, &dir.display().to_string()).await?;
        for track in db.list_tracks(m.id).await? {
            if let Some(file) = std::path::Path::new(&track.path).file_name() {
                db.set_track_path(track.id, &dir.join(file).display().to_string())
                    .await?;
            }
        }
    }
    Ok(())
}

/// Verify the engine and model; damaged files are removed so they download again (VRT-27).
/// Returns the names of the files that were damaged.
#[tauri::command]
pub async fn engine_check(
    engine: State<'_, EngineManager>,
    recorder: State<'_, RecorderState>,
    jobs: State<'_, Jobs>,
) -> Result<Vec<String>, String> {
    ensure_idle(&recorder, &jobs).await?;
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || engine.check_blocking())
        .await
        .map_err(|e| e.to_string())?
}

/// Delete the engine and model (frees ~1.1 GB); Download brings them back.
#[tauri::command]
pub async fn engine_remove(
    engine: State<'_, EngineManager>,
    recorder: State<'_, RecorderState>,
    jobs: State<'_, Jobs>,
) -> Result<(), String> {
    ensure_idle(&recorder, &jobs).await?;
    engine.remove()
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

    #[tokio::test]
    async fn moved_meetings_point_to_their_new_folder() {
        let db = Db::open_in_memory().await.unwrap();
        let from = crate::test_util::temp_dir().join("meetings");
        let dir = from.join("1");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("mic.flac"), b"x").unwrap();
        let m = db
            .create_meeting("m", 1, &dir.display().to_string())
            .await
            .unwrap();
        let track = NewTrack {
            source_kind: "mic",
            source_id: "mic",
            device_name: "Mic",
            path: &dir.join("mic.flac").display().to_string(),
            seconds: 1.0,
        };
        db.add_track(m, &track).await.unwrap();
        let to = crate::test_util::temp_dir().join("Elsewhere");
        crate::paths::move_meeting_folders(&from, &to).unwrap();
        point_meetings_to(&db, &to).await.unwrap();
        let moved = db.get_meeting(m).await.unwrap().unwrap();
        assert_eq!(std::path::Path::new(&moved.dir), to.join("1"));
        let t = &db.list_tracks(m).await.unwrap()[0];
        assert_eq!(
            std::fs::read(&t.path).unwrap(),
            b"x",
            "the track is found at its new place"
        );
    }

    fn block(label: &str, start_ms: u32, end_ms: u32) -> Labelled {
        Labelled {
            label: label.into(),
            start_ms,
            end_ms,
            text: "x".into(),
            words: vec![],
        }
    }

    #[tokio::test]
    async fn names_given_during_the_recording_survive_the_final_renumbering() {
        let db = Db::open_in_memory().await.unwrap();
        let m = db.create_meeting("m", 1, "/m").await.unwrap();
        let track = NewTrack {
            source_kind: "loopback",
            source_id: "out",
            device_name: "Out",
            path: "/m/system.flac",
            seconds: 9.0,
        };
        let t = db.add_track(m, &track).await.unwrap();
        let anna = db.create_speaker("Anna", 1).await.unwrap();
        // Rolling: Anna (named by the user) is "1", someone else "2".
        let rolling: Vec<NewSegment> = [block("1", 0, 5000), block("2", 5000, 9000)]
            .iter()
            .map(|b| new_segment(t, b))
            .collect();
        db.insert_segments(m, Pass::Rolling, &rolling)
            .await
            .unwrap();
        db.set_voice_identity(m, "1", Some(anna), None, None, true)
            .await
            .unwrap();
        let named = named_stretches(&db, m).await;
        assert_eq!(named, [(t, 0, 5000, anna)]);

        // The final pass numbers them the other way round.
        let blocks = vec![(t, block("2", 100, 4900)), (t, block("1", 5100, 8900))];
        let rows: Vec<NewSegment> = blocks.iter().map(|(id, b)| new_segment(*id, b)).collect();
        db.replace_segments(m, &[Pass::Rolling], 0, i64::MAX, Pass::Final, &rows)
            .await
            .unwrap();
        let voice = |label: &str| LabelVoice {
            label: label.into(),
            embedding: None,
            clean_seconds: 0.0,
            encoder_id: "sha256:e".into(),
        };
        relabel_final(&db, m, &blocks, &[voice("2"), voice("1")], &named)
            .await
            .unwrap();
        let voices = db.meeting_voices(m).await.unwrap();
        let who: HashMap<_, _> = voices
            .iter()
            .map(|v| (v.label.as_str(), (v.speaker_id, v.confirmed)))
            .collect();
        assert_eq!(who["2"], (Some(anna), true), "Anna is now label 2");
        assert_eq!(who["1"], (None, false));
        let segs = db.list_segments(m).await.unwrap();
        assert_eq!(segs[0].speaker_id, Some(anna));
        assert_eq!(segs[1].speaker_id, None);
    }

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
        assert_eq!(info.name, "Manent");
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

// ---- Nextcloud sync (VRT-39) ----

#[tauri::command]
pub fn sync_status(sync: State<'_, crate::sync::Sync>) -> crate::sync::SyncStatus {
    sync.status()
}

/// Start connecting: returns the login page (also opened in the browser).
#[tauri::command]
pub async fn sync_connect(
    sync: State<'_, crate::sync::Sync>,
    server: String,
) -> Result<String, String> {
    sync.connect(server).await
}

/// Open the login page of a connection in progress again (the browser window was closed).
#[tauri::command]
pub fn sync_open_login(sync: State<'_, crate::sync::Sync>) {
    if let Some(url) = sync.status().login_url {
        crate::open_url::open(&url);
    }
}

#[tauri::command]
pub fn sync_cancel_connect(sync: State<'_, crate::sync::Sync>) {
    sync.cancel_connect();
}

#[tauri::command]
pub async fn sync_disconnect(sync: State<'_, crate::sync::Sync>) -> Result<(), String> {
    sync.disconnect().await
}

#[tauri::command]
pub async fn sync_set_mode(sync: State<'_, crate::sync::Sync>, mode: String) -> Result<(), String> {
    sync.set_mode(&mode).await
}

/// A round now ("Sync now", or the window came to the front).
#[tauri::command]
pub fn sync_now() {
    crate::sync::wake();
}

/// The UI's UTC offset, for folder names in local time.
#[tauri::command]
pub fn set_utc_offset(minutes: i32) {
    crate::sync::set_utc_offset(minutes);
}
