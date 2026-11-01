//! Background transcription jobs (VRT-19): imported files, re-transcribing a meeting, and the
//! final pass after a recording. Jobs run one at a time on their own thread, so the engine is
//! never asked to transcribe two whole meetings at once; the UI follows them through the
//! `jobs:changed` event and they survive navigation (the state lives here, not in the UI).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::audio::{self, SourceKind};
use crate::commands::{
    KEEP_AUDIO_SETTING, PassEvent, SpeakersChanged, named_stretches, new_segment, now_ms,
    relabel_final, segment_view, waveform_bars,
};
use crate::db::{Db, NewSegment, NewTrack, Pass};
use crate::engine::{EngineManager, SAMPLE_RATE};
use crate::live::TrackRef;
use crate::media_server::MIX_FILE;
use crate::paths::AppPaths;
use crate::rolling;

/// A job as the UI shows it.
#[derive(Debug, Clone, Serialize)]
pub struct JobView {
    pub id: u64,
    /// "import", "transcribe" or "recover".
    pub kind: &'static str,
    /// The file or meeting name.
    pub name: String,
    pub meeting_id: Option<i64>,
    /// "waiting", "decoding", "transcribing", "done" or "failed".
    pub stage: &'static str,
    /// 0..1 (transcription is estimated from the audio length).
    pub progress: f64,
    pub error: Option<String>,
}

enum Work {
    Import(PathBuf),
    Transcribe(i64),
    Recover(i64),
}

/// Finished jobs stay listed (so the UI can show the outcome) until dismissed, up to this many.
const KEEP_FINISHED: usize = 20;
/// Transcription plus diarization of one track take about this much of the audio's length,
/// plus a little startup; used only to show progress. Measured in VRT-35 (release build,
/// 25-40 minutes): 0.105 for an imported track; a recording's two tracks (mic and system,
/// in parallel) 0.145, so each further track adds about EXTRA_TRACK_RTF.
const EXPECTED_RTF: f64 = 0.11;
const EXTRA_TRACK_RTF: f64 = 0.04;
/// Share of an import's progress bar for decoding (about 45 s of a 280 s job in VRT-35).
const DECODE_SHARE: f64 = 0.15;

struct Shared {
    app: Option<AppHandle>,
    db: Db,
    engine: EngineManager,
    paths: AppPaths,
    jobs: Mutex<Vec<JobView>>,
    next_id: AtomicU64,
    last_emit: Mutex<Instant>,
}

#[derive(Clone)]
pub struct Jobs {
    shared: Arc<Shared>,
    queue: mpsc::Sender<(u64, Work)>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Jobs {
    pub fn new(app: Option<AppHandle>, db: Db, engine: EngineManager, paths: AppPaths) -> Self {
        let shared = Arc::new(Shared {
            app,
            db,
            engine,
            paths,
            jobs: Mutex::new(Vec::new()),
            next_id: AtomicU64::new(1),
            last_emit: Mutex::new(Instant::now() - Duration::from_secs(1)),
        });
        let (queue, rx) = mpsc::channel::<(u64, Work)>();
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("veritas-jobs".into())
            .spawn(move || {
                for (id, work) in rx {
                    let result = match work {
                        Work::Import(path) => {
                            tauri::async_runtime::block_on(run_import(&worker, id, path))
                        }
                        Work::Transcribe(meeting_id) => {
                            tauri::async_runtime::block_on(run_transcribe(&worker, id, meeting_id))
                        }
                        Work::Recover(meeting_id) => {
                            tauri::async_runtime::block_on(run_recover(&worker, id, meeting_id))
                        }
                    };
                    match result {
                        Ok(()) => worker.update(id, |j| {
                            j.stage = "done";
                            j.progress = 1.0;
                        }),
                        Err(e) => {
                            eprintln!("job {id} failed: {e}");
                            worker.update(id, |j| {
                                j.stage = "failed";
                                j.error = Some(e.clone());
                            })
                        }
                    }
                }
            })
            .expect("start the job thread");
        Self { shared, queue }
    }

    fn add(&self, kind: &'static str, name: String, meeting_id: Option<i64>, work: Work) -> u64 {
        let id = self.shared.next_id.fetch_add(1, Ordering::Relaxed);
        {
            let mut jobs = lock(&self.shared.jobs);
            jobs.push(JobView {
                id,
                kind,
                name,
                meeting_id,
                stage: "waiting",
                progress: 0.0,
                error: None,
            });
            let finished = jobs
                .iter()
                .filter(|j| matches!(j.stage, "done" | "failed"))
                .count();
            if finished > KEEP_FINISHED
                && let Some(i) = jobs
                    .iter()
                    .position(|j| matches!(j.stage, "done" | "failed"))
            {
                jobs.remove(i);
            }
        }
        let _ = self.queue.send((id, work));
        self.shared.emit(true);
        id
    }

    /// Import a recording made elsewhere: it becomes a meeting with a transcript.
    pub fn import(&self, path: PathBuf) -> u64 {
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        self.add("import", name, None, Work::Import(path))
    }

    /// Transcribe a meeting (again): its tracks are transcribed and diarized whole, replacing
    /// every segment (the final pass after a recording, or "Re-transcribe").
    pub fn transcribe(&self, meeting_id: i64, name: String) -> u64 {
        self.add(
            "transcribe",
            name,
            Some(meeting_id),
            Work::Transcribe(meeting_id),
        )
    }

    /// Recover a recording cut off when the app died (VRT-41): repair its tracks, make the
    /// playback file, then transcribe it.
    pub fn recover(&self, meeting_id: i64, name: String) -> u64 {
        self.add("recover", name, Some(meeting_id), Work::Recover(meeting_id))
    }

    pub fn list(&self) -> Vec<JobView> {
        lock(&self.shared.jobs).clone()
    }

    /// Forget a finished job.
    pub fn dismiss(&self, id: u64) {
        lock(&self.shared.jobs).retain(|j| j.id != id || !matches!(j.stage, "done" | "failed"));
        self.shared.emit(true);
    }

    /// Whether any job is waiting or running.
    pub fn any_running(&self) -> bool {
        lock(&self.shared.jobs)
            .iter()
            .any(|j| !matches!(j.stage, "done" | "failed"))
    }

    /// Whether a meeting has a job waiting or running.
    pub fn busy(&self, meeting_id: i64) -> bool {
        lock(&self.shared.jobs)
            .iter()
            .any(|j| j.meeting_id == Some(meeting_id) && !matches!(j.stage, "done" | "failed"))
    }
}

impl Shared {
    fn update(&self, id: u64, f: impl FnOnce(&mut JobView)) {
        let stage_changed = {
            let mut jobs = lock(&self.jobs);
            let Some(job) = jobs.iter_mut().find(|j| j.id == id) else {
                return;
            };
            let before = job.stage;
            f(job);
            before != job.stage
        };
        self.emit(stage_changed);
    }

    /// `jobs:changed`, at most 4 times a second for progress-only updates.
    fn emit(&self, force: bool) {
        {
            let mut last = lock(&self.last_emit);
            if !force && last.elapsed() < Duration::from_millis(250) {
                return;
            }
            *last = Instant::now();
        }
        if let Some(app) = &self.app {
            let _ = app.emit("jobs:changed", lock(&self.jobs).clone());
        }
    }
}

async fn run_import(s: &Shared, id: u64, path: PathBuf) -> Result<(), String> {
    let err = |e: crate::db::DbError| e.to_string();
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let title = path
        .file_stem()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    // The meeting dates from the file (when it was recorded or saved), else from now.
    let started_at = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(now_ms()?, |d| d.as_millis() as i64);
    let pending = s.paths.meetings_root().join(format!("import-{id}"));
    let meeting_id =
        s.db.create_meeting(&title, started_at, &pending.display().to_string())
            .await
            .map_err(err)?;
    let dir = s.paths.meetings_root().join(meeting_id.to_string());
    let cleanup = |reason: String| async move {
        let _ = s.db.delete_meeting(meeting_id).await;
        let _ = std::fs::remove_dir_all(s.paths.meetings_root().join(meeting_id.to_string()));
        Err::<(), String>(reason)
    };
    s.db.set_meeting_dir(meeting_id, &dir.display().to_string())
        .await
        .map_err(err)?;
    s.db.finish_meeting(meeting_id, 0, "processing")
        .await
        .map_err(err)?;
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return cleanup(e.to_string()).await;
    }
    s.update(id, |j| {
        j.meeting_id = Some(meeting_id);
        j.stage = "decoding";
    });
    let track_file = dir.join("import.flac");
    let track = NewTrack {
        source_kind: "loopback",
        source_id: "import",
        device_name: &name,
        path: &track_file.display().to_string(),
        seconds: 0.0,
    };
    let track_id = s.db.add_track(meeting_id, &track).await.map_err(err)?;
    let decoded = {
        let (src, dest) = (path.clone(), track_file.clone());
        let progress_target = SharedProgress::new();
        let p = progress_target.clone();
        let handle = tauri::async_runtime::spawn_blocking(move || {
            audio::import_to_flac(&src, &dest, &mut |f| p.set(f))
        });
        tokio::pin!(handle);
        // Report decoding progress while it runs (the first DECODE_SHARE of the job).
        let result = loop {
            tokio::select! {
                r = &mut handle => break r,
                () = tokio::time::sleep(Duration::from_millis(200)) => {
                    s.update(id, |j| j.progress = DECODE_SHARE * progress_target.get());
                }
            }
        };
        result.map_err(|e| e.to_string())
    };
    let seconds = match decoded {
        Ok(Ok(seconds)) => seconds,
        Ok(Err(e)) => return cleanup(e.to_string()).await,
        Err(e) => return cleanup(e).await,
    };
    s.db.set_track_seconds(track_id, seconds)
        .await
        .map_err(err)?;
    s.db.finish_meeting(meeting_id, (seconds * 1000.0).round() as i64, "processing")
        .await
        .map_err(err)?;
    // Playback file, as for a recording; a failure leaves the meeting without a player only.
    let (files, mix) = (vec![track_file], dir.join(MIX_FILE));
    if let Ok(Err(e)) =
        tauri::async_runtime::spawn_blocking(move || audio::mixdown(&files, &mix)).await
    {
        eprintln!("mixdown of import {meeting_id} failed: {e}");
    }
    transcribe(s, id, meeting_id, DECODE_SHARE).await
}

/// A meeting still marked "recording" at startup: the app died while recording it. Its FLAC
/// tracks lack their final header and the last moments; keep every complete frame.
async fn run_recover(s: &Shared, id: u64, meeting_id: i64) -> Result<(), String> {
    let err = |e: crate::db::DbError| e.to_string();
    s.update(id, |j| j.stage = "decoding");
    let Some(meeting) = s.db.get_meeting(meeting_id).await.map_err(err)? else {
        return Ok(());
    };
    let mut files = Vec::new();
    let mut longest: f64 = 0.0;
    for t in s.db.list_tracks(meeting_id).await.map_err(err)? {
        let path = PathBuf::from(&t.path);
        let repaired = {
            let path = path.clone();
            tauri::async_runtime::spawn_blocking(move || audio::repair_flac(&path))
                .await
                .map_err(|e| e.to_string())?
        };
        match repaired {
            Ok(seconds) if seconds > 0.0 => {
                s.db.set_track_seconds(t.id, seconds).await.map_err(err)?;
                longest = longest.max(seconds);
                files.push(path);
            }
            other => {
                // Nothing usable in it (the crash came before any audio reached the disk).
                if let Err(e) = other {
                    eprintln!("recovering meeting {meeting_id}: {}: {e}", t.path);
                }
                let _ = std::fs::remove_file(&path);
                s.db.delete_track(t.id).await.map_err(err)?;
            }
        }
    }
    if files.is_empty() {
        // No audio at all: nothing to keep.
        s.db.delete_meeting(meeting_id).await.map_err(err)?;
        let _ = std::fs::remove_dir_all(&meeting.dir);
        eprintln!("recovering meeting {meeting_id}: no audio was saved; removed");
        return Ok(());
    }
    s.db.set_interrupted(meeting_id).await.map_err(err)?;
    let duration_ms = (longest * 1000.0).round() as i64;
    s.db.finish_meeting(meeting_id, duration_ms, "processing")
        .await
        .map_err(err)?;
    let mix = PathBuf::from(&meeting.dir).join(MIX_FILE);
    if let Ok(Err(e)) =
        tauri::async_runtime::spawn_blocking(move || audio::mixdown(&files, &mix)).await
    {
        eprintln!("mixdown of recovered meeting {meeting_id} failed: {e}");
    }
    eprintln!("recovered meeting {meeting_id}: {longest:.1} s kept");
    let status = s.engine.status();
    if !(status.engine_installed && status.model_installed) {
        // Without the engine it stays untranscribed, as a recording made without it.
        s.db.finish_meeting(meeting_id, duration_ms, "done")
            .await
            .map_err(err)?;
        return Ok(());
    }
    transcribe(s, id, meeting_id, DECODE_SHARE).await
}

async fn run_transcribe(s: &Shared, id: u64, meeting_id: i64) -> Result<(), String> {
    transcribe(s, id, meeting_id, 0.0).await
}

/// A fraction shared between a blocking task and the job loop.
#[derive(Clone)]
struct SharedProgress(Arc<AtomicU64>);

impl SharedProgress {
    fn new() -> Self {
        Self(Arc::new(AtomicU64::new(0)))
    }
    fn set(&self, f: f64) {
        self.0.store(f.to_bits(), Ordering::Relaxed);
    }
    fn get(&self) -> f64 {
        f64::from_bits(self.0.load(Ordering::Relaxed))
    }
}

/// Transcribe a meeting's tracks whole (from progress `base` to 100 %): words and speakers,
/// bleed removed, names carried over from earlier passes, people recognised; then 'done'.
async fn transcribe(s: &Shared, id: u64, meeting_id: i64, base: f64) -> Result<(), String> {
    let err = |e: crate::db::DbError| e.to_string();
    let status = s.engine.status();
    if !(status.engine_installed && status.model_installed) {
        let _ =
            s.db.finish_meeting(meeting_id, meeting_duration(s, meeting_id).await, "done")
                .await;
        return Err("Download the transcription engine in Settings first".into());
    }
    let tracks = s.db.list_tracks(meeting_id).await.map_err(err)?;
    let duration_ms = meeting_duration(s, meeting_id).await;
    s.db.finish_meeting(meeting_id, duration_ms, "processing")
        .await
        .map_err(err)?;
    s.update(id, |j| {
        j.stage = "transcribing";
        j.progress = base;
    });
    let refs: Vec<(TrackRef, PathBuf)> = tracks
        .iter()
        .map(|t| {
            (
                TrackRef {
                    meeting_id,
                    track_id: t.id,
                    source_id: t.source_id.clone(),
                    kind: if t.source_kind == "mic" {
                        SourceKind::Mic
                    } else {
                        SourceKind::Loopback
                    },
                },
                PathBuf::from(&t.path),
            )
        })
        .collect();
    let kinds: HashMap<i64, SourceKind> = refs.iter().map(|(t, _)| (t.track_id, t.kind)).collect();

    // Progress is estimated from the audio length while the engine works.
    let done = Arc::new(AtomicBool::new(false));
    let ticker = {
        let (done, expected) = (
            done.clone(),
            Duration::from_secs_f64(
                duration_ms as f64 / 1000.0
                    * (EXPECTED_RTF + EXTRA_TRACK_RTF * refs.len().saturating_sub(1) as f64)
                    + 3.0,
            ),
        );
        let started = Instant::now();
        async move {
            while !done.load(Ordering::Relaxed) {
                let f = (started.elapsed().as_secs_f64() / expected.as_secs_f64()).min(0.95);
                s.update(id, |j| j.progress = base + (1.0 - base) * f);
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    };
    let work = async {
        let decoded = tauri::async_runtime::spawn_blocking(move || {
            refs.into_iter()
                .map(|(t, file)| {
                    audio::decode_mono(&file, SAMPLE_RATE)
                        .map(|samples| (t, samples))
                        .map_err(|e| e.to_string())
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .await
        .map_err(|e| e.to_string())??;
        let result = rolling::final_pass(&s.engine, decoded, &s.paths.tmp_dir()).await;
        done.store(true, Ordering::Relaxed);
        result
    };
    let (result, ()) = tokio::join!(work, ticker);
    let (blocks, voices) = match result {
        Ok(r) => r,
        Err(e) => {
            // The meeting keeps whatever transcript it had.
            let _ = s.db.finish_meeting(meeting_id, duration_ms, "done").await;
            return Err(e);
        }
    };
    let named = named_stretches(&s.db, meeting_id).await;
    let rows: Vec<NewSegment> = blocks.iter().map(|(t, b)| new_segment(*t, b)).collect();
    s.db.replace_segments(
        meeting_id,
        &[Pass::Caption, Pass::Rolling, Pass::Final],
        0,
        i64::MAX,
        Pass::Final,
        &rows,
    )
    .await
    .map_err(err)?;
    relabel_final(&s.db, meeting_id, &blocks, &voices, &named).await?;
    s.db.finish_meeting(meeting_id, duration_ms, "done")
        .await
        .map_err(err)?;
    let keep: Option<String> = s.db.get_setting(KEEP_AUDIO_SETTING).await.map_err(err)?;
    if keep.as_deref() == Some("mix") {
        drop_tracks(s, meeting_id).await;
    }
    // transcript.md next to the audio (VRT-37), after the tracks it lists may have gone.
    if let Err(e) = crate::transcript_md::write(&s.db, meeting_id).await {
        eprintln!("transcript.md for meeting {meeting_id}: {e}");
    }
    if let Some(app) = &s.app {
        let segments = blocks
            .into_iter()
            .map(|(t, b)| segment_view(meeting_id, t, kinds[&t], "final", b))
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
        let _ = app.emit(
            "speakers:changed",
            SpeakersChanged {
                meeting_id: Some(meeting_id),
            },
        );
    }
    Ok(())
}

/// Settings > Storage "playback file only" (VRT-27): once transcribed, the FLAC tracks go and
/// only mix.opus (plus the cached waveform) stays. Never without a playback file.
async fn drop_tracks(s: &Shared, meeting_id: i64) {
    let Ok(Some(meeting)) = s.db.get_meeting(meeting_id).await else {
        return;
    };
    if !std::path::Path::new(&meeting.dir).join(MIX_FILE).is_file() {
        return;
    }
    if let Err(e) = waveform_bars(&s.db, &s.paths, meeting_id).await {
        eprintln!("waveform of meeting {meeting_id} before dropping its tracks: {e}");
        return;
    }
    for t in s.db.list_tracks(meeting_id).await.unwrap_or_default() {
        let _ = std::fs::remove_file(&t.path);
    }
}

/// The meeting's length: its longest track.
async fn meeting_duration(s: &Shared, meeting_id: i64) -> i64 {
    s.db.list_tracks(meeting_id)
        .await
        .map(|t| t.iter().map(|t| t.seconds).fold(0.0, f64::max))
        .map_or(0, |sec| (sec * 1000.0).round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn setup() -> (Jobs, Db, AppPaths) {
        let paths = AppPaths::new(crate::test_util::temp_dir());
        std::fs::create_dir_all(paths.meetings_root()).unwrap();
        let db = tauri::async_runtime::block_on(Db::open_in_memory()).unwrap();
        // No engine downloaded in this folder.
        let engine = EngineManager::new(paths.clone(), |_| {});
        (
            Jobs::new(None, db.clone(), engine, paths.clone()),
            db,
            paths,
        )
    }

    fn wait(jobs: &Jobs, id: u64) -> JobView {
        for _ in 0..200 {
            let job = jobs.list().into_iter().find(|j| j.id == id).unwrap();
            if matches!(job.stage, "done" | "failed") {
                return job;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("job {id} did not finish");
    }

    #[test]
    fn an_unreadable_file_fails_and_leaves_no_meeting() {
        let (jobs, db, paths) = setup();
        let file = paths.data.join("notes.mp3");
        std::fs::write(&file, b"not audio").unwrap();
        let job = wait(&jobs, jobs.import(file));
        assert_eq!(job.stage, "failed");
        assert!(job.error.unwrap().contains("can read"), "a clear reason");
        let meetings = tauri::async_runtime::block_on(db.list_meetings()).unwrap();
        assert!(meetings.is_empty());
        assert_eq!(std::fs::read_dir(paths.meetings_root()).unwrap().count(), 0);
        jobs.dismiss(job.id);
        assert!(jobs.list().is_empty());
    }

    #[test]
    fn an_import_becomes_a_playable_meeting_even_without_the_engine() {
        let (jobs, db, _) = setup();
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/import/tone.m4a");
        let job = wait(&jobs, jobs.import(file));
        assert_eq!(job.stage, "failed", "transcription needs the engine");
        assert!(
            job.error
                .as_deref()
                .unwrap_or_default()
                .contains("Download")
        );
        let id = job.meeting_id.unwrap();
        let m = tauri::async_runtime::block_on(db.get_meeting(id))
            .unwrap()
            .unwrap();
        assert_eq!((m.title.as_str(), m.status.as_str()), ("tone", "done"));
        assert!((m.duration_ms - 2000).abs() < 100, "{}", m.duration_ms);
        let tracks = tauri::async_runtime::block_on(db.list_tracks(id)).unwrap();
        assert_eq!(tracks.len(), 1);
        assert!(Path::new(&tracks[0].path).is_file());
        assert!(
            Path::new(&m.dir).join(MIX_FILE).is_file(),
            "playback file made"
        );
        assert!(!jobs.busy(id));
    }
}
