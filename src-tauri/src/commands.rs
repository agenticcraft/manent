//! Tauri commands exposed to the UI. Keep in sync with `src/lib/ipc.ts`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::audio::{self, Monitor, Recording, SourceInfo, SourceKind, TrackSummary};
use crate::db::Pass;
use crate::db::{Db, Meeting, NewSegment, NewTrack, Speaker};
use crate::engine::transcript::Word;
use crate::engine::{EngineManager, EngineStatus};
use crate::live::{self, Caption, SharedHistory, TrackRef};
use crate::media_server::{MIX_FILE, MediaServer};
use crate::passes::Labelled;
use crate::paths::AppPaths;
use crate::rolling::{self, PassResult, PassTrack, Rolling};

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
    /// Database rows of the tracks, in source order (created at start, VRT-16).
    track_ids: Vec<i64>,
    captions: bool,
    /// Live transcription: caption workers (VRT-16) and rolling passes (VRT-17).
    workers: Workers,
    rolling: Option<Rolling>,
}

/// The caption workers of a recording and the track audio they keep for the passes.
type Workers = Arc<Mutex<Vec<(PassTrack, std::thread::JoinHandle<()>)>>>;

/// The recording in progress, if any.
#[derive(Default)]
pub struct RecorderState(tokio::sync::Mutex<Option<ActiveRecording>>);

#[derive(Debug, Clone, Serialize)]
pub struct RecordingStarted {
    pub meeting_id: i64,
    /// Unix epoch milliseconds.
    pub started_at: i64,
    pub sources: Vec<SourceInfo>,
    /// Live captions are on (the engine and model are downloaded).
    pub captions: bool,
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
        passes = start_rolling(&app, db.inner(), engine.inner(), &paths, meeting_id, tracks);
    }
    *current = Some(ActiveRecording {
        recording,
        meeting_id,
        started_at,
        sources: sources.clone(),
        track_ids,
        captions,
        workers,
        rolling: passes,
    });
    Ok(RecordingStarted {
        meeting_id,
        started_at,
        sources,
        captions,
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
        captions: a.captions,
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
    app: AppHandle,
    state: State<'_, RecorderState>,
    db: State<'_, Db>,
    engine: State<'_, EngineManager>,
    paths: State<'_, AppPaths>,
) -> Result<RecordingStopped, String> {
    let ActiveRecording {
        recording,
        meeting_id,
        track_ids,
        workers,
        rolling,
        ..
    } = state.0.lock().await.take().ok_or("no recording")?;
    let tracks = tauri::async_runtime::spawn_blocking(move || recording.stop())
        .await
        .map_err(|e| e.to_string())?
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
    let duration_ms = duration_ms.round() as i64;
    // With transcription on, the final pass runs in the background ("processing").
    let final_tracks: Vec<(TrackRef, std::path::PathBuf)> = pass_tracks
        .into_iter()
        .filter_map(|t| {
            let i = track_ids.iter().position(|id| *id == t.track_id)?;
            Some((t, tracks.get(i)?.file.clone()))
        })
        .collect();
    let status = if final_tracks.is_empty() {
        "done"
    } else {
        "processing"
    };
    db.finish_meeting(meeting_id, duration_ms, status)
        .await
        .map_err(|e| e.to_string())?;
    if !final_tracks.is_empty() {
        start_final_pass(
            app,
            db.inner().clone(),
            engine.inner().clone(),
            paths.tmp_dir(),
            meeting_id,
            duration_ms,
            final_tracks,
        );
    }
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
        .map_err(|e| e.to_string())
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn new_segment(track_id: i64, b: &Labelled) -> NewSegment {
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

fn segment_view(
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
    match rolling::spawn(tracks, engine.clone(), paths.tmp_dir(), on_pass, on_error) {
        Ok(r) => Some(r),
        Err(e) => {
            eprintln!("starting the rolling passes failed: {e}");
            None
        }
    }
}

/// The final pass after a recording (VRT-17): transcribe and diarize the whole tracks, replace
/// every caption and rolling segment, and mark the meeting done.
fn start_final_pass(
    app: AppHandle,
    db: Db,
    engine: EngineManager,
    tmp: std::path::PathBuf,
    meeting_id: i64,
    duration_ms: i64,
    tracks: Vec<(TrackRef, std::path::PathBuf)>,
) {
    tauri::async_runtime::spawn(async move {
        let started = std::time::Instant::now();
        let kinds: HashMap<i64, SourceKind> =
            tracks.iter().map(|(t, _)| (t.track_id, t.kind)).collect();
        let decoded = tauri::async_runtime::spawn_blocking(move || {
            tracks
                .into_iter()
                .map(|(t, file)| {
                    audio::decode_mono(&file, crate::engine::SAMPLE_RATE)
                        .map(|s| (t, s))
                        .map_err(|e| e.to_string())
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r);
        let result = match decoded {
            Ok(tracks) => rolling::final_pass(&engine, tracks, &tmp).await,
            Err(e) => Err(e),
        };
        match result {
            Ok(blocks) => {
                let rows: Vec<NewSegment> =
                    blocks.iter().map(|(id, b)| new_segment(*id, b)).collect();
                let saved = db
                    .replace_segments(
                        meeting_id,
                        &[Pass::Caption, Pass::Rolling, Pass::Final],
                        0,
                        i64::MAX,
                        Pass::Final,
                        &rows,
                    )
                    .await;
                if let Err(e) = saved {
                    eprintln!("saving the final pass failed: {e}");
                }
                eprintln!(
                    "meeting {meeting_id}: final pass done in {:.1} s",
                    started.elapsed().as_secs_f64()
                );
                let segments = blocks
                    .into_iter()
                    .map(|(id, b)| segment_view(meeting_id, id, kinds[&id], "final", b))
                    .collect();
                let _ = app.emit(
                    "transcript:final",
                    PassEvent {
                        meeting_id,
                        from_ms: 0,
                        to_ms: duration_ms as u32,
                        segments,
                    },
                );
            }
            Err(message) => {
                // The captions and rolling segments stay; the meeting is still usable.
                eprintln!("final pass failed for meeting {meeting_id}: {message}");
                let _ = app.emit(
                    "transcript:error",
                    CaptionError {
                        meeting_id,
                        message,
                    },
                );
            }
        }
        if let Err(e) = db.finish_meeting(meeting_id, duration_ms, "done").await {
            eprintln!("marking meeting {meeting_id} done failed: {e}");
        }
    });
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
