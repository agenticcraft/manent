//! Push-to-talk dictation (VRT-58, decision-11): hold the dictation shortcut, speak, release.
//! Manent records the mic only, transcribes it with the local engine and puts the text on the
//! clipboard; a notification says it is ready to paste. Nothing is stored: no meeting, no
//! speakers, and the audio stays in memory (the engine helper reads it from a WAV file in the
//! user's runtime directory, which is RAM, and that file is removed straight after).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::audio::{self, AudioSource, CpalSource, SourceKind};
use crate::commands::{RecorderState, SOURCES_SETTING, resolve_sources};
use crate::db::Db;
use crate::engine::{self, Engine, EngineManager};
use crate::live::downmix;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(not(target_os = "linux"))]
mod other;
#[cfg(not(target_os = "linux"))]
use other as platform;

/// A press shorter than this is a slip of the finger, not dictation.
pub const MIN_HOLD: Duration = Duration::from_millis(300);
/// Dictation stops by itself after this long, in case the release never arrives.
pub const MAX_HOLD: Duration = Duration::from_secs(5 * 60);
/// How often the capture thread empties the mic's ring.
const POLL: Duration = Duration::from_millis(20);
/// The notification's id: each one replaces the last.
const NOTIFICATION: &str = "dictation";

/// What a press of the dictation key does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Start listening (this dictation's number).
    Listen(u64),
    /// A meeting is being recorded: dictation isn't available (decision-11).
    Blocked,
    /// Still listening or transcribing the last dictation.
    Busy,
}

/// What a release (or the time limit) does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Release {
    /// Stop listening and transcribe.
    Transcribe,
    /// Released too soon: stop listening, do nothing.
    TooShort,
    /// Not listening (or a different dictation's time limit).
    Ignore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Listening { id: u64, since: Instant },
    Transcribing,
}

/// The hold logic: press starts listening, release stops it; repeats are filtered out before.
#[derive(Debug)]
pub struct Hold {
    state: State,
    next: u64,
}

impl Default for Hold {
    fn default() -> Self {
        Hold {
            state: State::Idle,
            next: 1,
        }
    }
}

impl Hold {
    pub fn press(&mut self, now: Instant, recording_meeting: bool) -> Press {
        match self.state {
            State::Idle if recording_meeting => Press::Blocked,
            State::Idle => {
                let id = self.next;
                self.next += 1;
                self.state = State::Listening { id, since: now };
                Press::Listen(id)
            }
            _ => Press::Busy,
        }
    }

    /// The key went up.
    pub fn release(&mut self, now: Instant) -> Release {
        match self.state {
            State::Listening { since, .. } if now.duration_since(since) < MIN_HOLD => {
                self.state = State::Idle;
                Release::TooShort
            }
            State::Listening { .. } => {
                self.state = State::Transcribing;
                Release::Transcribe
            }
            _ => Release::Ignore,
        }
    }

    /// Dictation `id` reached MAX_HOLD.
    pub fn time_up(&mut self, id: u64) -> Release {
        match self.state {
            State::Listening { id: current, .. } if current == id => {
                self.state = State::Transcribing;
                Release::Transcribe
            }
            _ => Release::Ignore,
        }
    }

    /// Listening failed before it started (no mic, no engine).
    pub fn abort(&mut self) {
        self.state = State::Idle;
    }

    /// The text is on the clipboard (or it failed): ready for the next one.
    pub fn done(&mut self) {
        self.state = State::Idle;
    }
}

/// The mic, recording into memory on its own thread.
struct Capture {
    stop: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<Result<Vec<f32>, String>>,
}

impl Capture {
    fn start(db: Db) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let thread = std::thread::Builder::new()
            .name("manent-dictation".into())
            .spawn(move || capture(&db, &stopped))
            .map_err(|e| e.to_string())?;
        Ok(Capture { stop, thread })
    }

    /// Stop and return the audio: 16 kHz mono.
    fn finish(self) -> Result<Vec<f32>, String> {
        self.stop.store(true, Ordering::Relaxed);
        self.thread
            .join()
            .map_err(|_| "the microphone thread failed".to_string())?
    }
}

/// Record the mic chosen in Settings > Sources (or the default one) until `stop`, as 16 kHz
/// mono.
fn capture(db: &Db, stop: &AtomicBool) -> Result<Vec<f32>, String> {
    let configured: Option<Vec<String>> =
        tauri::async_runtime::block_on(db.get_setting(SOURCES_SETTING))
            .ok()
            .flatten();
    let available = audio::list_sources().map_err(|e| e.to_string())?;
    let mic = |configured: Option<&[String]>| {
        resolve_sources(&available, configured)
            .into_iter()
            .find(|s| s.kind == SourceKind::Mic)
    };
    // A configuration without a mic still means the default mic.
    let mic = mic(configured.as_deref())
        .or_else(|| mic(None))
        .ok_or("no microphone found")?;
    let (mut source, mut ring) = CpalSource::open(mic).map_err(|e| e.to_string())?;
    let format = source.format();
    source.start().map_err(|e| e.to_string())?;
    let mut resampler =
        audio::resample::StreamResampler::new(format.sample_rate, engine::SAMPLE_RATE);
    let (mut raw, mut mono, mut out) = (Vec::new(), Vec::new(), Vec::new());
    let mut drain = |ring: &mut audio::RingReader, out: &mut Vec<f32>| {
        ring.drain_into(&mut raw);
        downmix(&raw, format.channels, &mut mono);
        resampler.process(&mono, out);
        raw.clear();
        mono.clear();
    };
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(POLL);
        drain(&mut ring, &mut out);
    }
    let _ = source.stop();
    drain(&mut ring, &mut out);
    resampler.finish(&mut out);
    Ok(out)
}

/// Dictation's state (managed).
#[derive(Default)]
pub struct Dictation {
    hold: Mutex<Hold>,
    capture: Mutex<Option<Capture>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Show a notification (replacing the previous dictation notification).
fn notify(title: &str, body: &str) {
    eprintln!("dictation: notification: {title}");
    let (title, body) = (title.to_string(), body.to_string());
    tauri::async_runtime::spawn(async move {
        if let Err(e) = platform::notify(NOTIFICATION, &title, &body).await {
            eprintln!("dictation: notification failed: {e}");
        }
    });
}

/// The dictation key went down.
pub fn pressed(app: &AppHandle) {
    let state = app.state::<Dictation>();
    let recording = app.state::<RecorderState>().is_recording();
    let press = lock(&state.hold).press(Instant::now(), recording);
    let id = match press {
        Press::Listen(id) => id,
        Press::Blocked => {
            notify(
                "Dictation isn't available while recording",
                "Stop the meeting recording to dictate.",
            );
            return;
        }
        Press::Busy => return,
    };
    let status = app.state::<EngineManager>().status();
    if !(status.engine_installed && status.model_installed) {
        lock(&state.hold).abort();
        notify(
            "Dictation needs the transcription engine",
            "Download it in Manent's Settings > Transcription.",
        );
        return;
    }
    match Capture::start(app.state::<Db>().inner().clone()) {
        Ok(c) => *lock(&state.capture) = Some(c),
        Err(e) => {
            lock(&state.hold).abort();
            notify("Dictation failed", &e);
            return;
        }
    }
    crate::tray::set_dictating(app, true);
    // Stop by itself if the release never comes.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(MAX_HOLD).await;
        let release = lock(&app.state::<Dictation>().hold).time_up(id);
        if release == Release::Transcribe {
            finish(app);
        }
    });
}

/// The dictation key went up.
pub fn released(app: &AppHandle) {
    let state = app.state::<Dictation>();
    let release = lock(&state.hold).release(Instant::now());
    match release {
        Release::Transcribe => finish(app.clone()),
        Release::TooShort => {
            let capture = lock(&state.capture).take();
            crate::tray::set_dictating(app, false);
            if let Some(c) = capture {
                std::thread::spawn(move || drop(c.finish()));
            }
        }
        Release::Ignore => {}
    }
}

/// Stop listening, transcribe and put the text on the clipboard.
fn finish(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let capture = lock(&app.state::<Dictation>().capture).take();
        crate::tray::set_dictating(&app, false);
        let result = match capture {
            Some(c) => match tauri::async_runtime::spawn_blocking(move || c.finish()).await {
                Ok(r) => r,
                Err(e) => Err(e.to_string()),
            },
            None => Err("the microphone wasn't recording".into()),
        };
        let outcome = match result {
            Ok(samples) => transcribe_and_copy(&app, &samples).await,
            Err(e) => Err(e),
        };
        match outcome {
            Ok(Some(text)) => notify("Ready to paste", &preview(&text)),
            Ok(None) => notify("Nothing was heard", "Hold the key while you speak."),
            Err(e) => notify("Dictation failed", &e),
        }
        lock(&app.state::<Dictation>().hold).done();
    });
}

/// Where the audio goes for the engine: the runtime directory is in memory (tmpfs).
fn wav_path(app: &AppHandle) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|d| d.is_dir())
        .unwrap_or_else(|| app.state::<crate::paths::AppPaths>().tmp_dir());
    dir.join(format!(
        "manent-dictation-{}-{}.wav",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

async fn transcribe_and_copy(app: &AppHandle, samples: &[f32]) -> Result<Option<String>, String> {
    let started = Instant::now();
    let wav = wav_path(app);
    if let Some(dir) = wav.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    engine::write_wav(&wav, samples)?;
    let engine = app.state::<EngineManager>().inner().clone();
    let result = engine.transcribe_words(&wav).await;
    let _ = std::fs::remove_file(&wav);
    let text = result?.text.trim().to_string();
    eprintln!(
        "dictation: {:.1} s of speech transcribed in {} ms ({} characters)",
        samples.len() as f64 / engine::SAMPLE_RATE as f64,
        started.elapsed().as_millis(),
        text.chars().count()
    );
    if text.is_empty() {
        return Ok(None);
    }
    platform::copy(&text).await?;
    Ok(Some(text))
}

/// The start of the text, for the notification.
pub fn preview(text: &str) -> String {
    const MAX: usize = 120;
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= MAX {
        return flat;
    }
    let cut: String = flat.chars().take(MAX).collect();
    format!("{}…", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(t0: Instant, ms: u64) -> Instant {
        t0 + Duration::from_millis(ms)
    }

    #[test]
    fn hold_and_release_transcribes() {
        let mut h = Hold::default();
        let t0 = Instant::now();
        let Press::Listen(_) = h.press(t0, false) else {
            panic!("should listen")
        };
        assert_eq!(h.release(at(t0, 2000)), Release::Transcribe);
        // A second release (or a stray one) does nothing.
        assert_eq!(h.release(at(t0, 2100)), Release::Ignore);
        h.done();
        assert!(matches!(h.press(at(t0, 3000), false), Press::Listen(_)));
    }

    #[test]
    fn a_short_press_does_nothing() {
        let mut h = Hold::default();
        let t0 = Instant::now();
        h.press(t0, false);
        assert_eq!(h.release(at(t0, 299)), Release::TooShort);
        assert!(matches!(h.press(at(t0, 400), false), Press::Listen(_)));
        assert_eq!(h.release(at(t0, 700)), Release::Transcribe);
    }

    #[test]
    fn presses_while_busy_are_ignored() {
        let mut h = Hold::default();
        let t0 = Instant::now();
        h.press(t0, false);
        assert_eq!(h.press(at(t0, 100), false), Press::Busy);
        h.release(at(t0, 1000));
        // Still transcribing.
        assert_eq!(h.press(at(t0, 1100), false), Press::Busy);
        assert_eq!(h.release(at(t0, 1200)), Release::Ignore);
    }

    #[test]
    fn blocked_during_a_meeting_recording() {
        let mut h = Hold::default();
        let t0 = Instant::now();
        assert_eq!(h.press(t0, true), Press::Blocked);
        assert_eq!(h.release(at(t0, 1000)), Release::Ignore);
        assert!(matches!(h.press(at(t0, 2000), false), Press::Listen(_)));
    }

    #[test]
    fn the_time_limit_only_ends_its_own_dictation() {
        let mut h = Hold::default();
        let t0 = Instant::now();
        let Press::Listen(first) = h.press(t0, false) else {
            panic!()
        };
        h.release(at(t0, 1000));
        h.done();
        let Press::Listen(second) = h.press(at(t0, 2000), false) else {
            panic!()
        };
        assert_eq!(h.time_up(first), Release::Ignore);
        assert_eq!(h.time_up(second), Release::Transcribe);
        assert_eq!(h.release(at(t0, 3000)), Release::Ignore);
    }

    #[test]
    fn an_aborted_start_frees_the_key() {
        let mut h = Hold::default();
        let t0 = Instant::now();
        h.press(t0, false);
        h.abort();
        assert_eq!(h.release(at(t0, 1000)), Release::Ignore);
        assert!(matches!(h.press(at(t0, 1100), false), Press::Listen(_)));
    }

    #[test]
    fn previews_are_one_short_line() {
        assert_eq!(preview("Hallo\nwereld "), "Hallo wereld");
        let long = "woord ".repeat(50);
        let p = preview(&long);
        assert!(p.ends_with('…'));
        assert!(p.chars().count() <= 121);
    }
}
