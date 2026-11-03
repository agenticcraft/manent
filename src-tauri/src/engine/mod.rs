//! Transcription engine (doc-2 §1.3, decision-1): libparakeet in the `manent-engine`
//! helper process. This module downloads the engine and the model (VRT-14), starts the
//! helper on demand, restarts it after a crash, and implements the `Engine` trait (VRT-15).

pub mod assets;
pub mod download;
pub mod helper;
// File transcription and speakers are used by the rolling and final passes (VRT-17).
#[allow(dead_code)]
pub mod transcript;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;

use crate::paths::AppPaths;
use helper::{Helper, Launch, ReadyInfo};
pub use transcript::{Diarization, Segment, Transcript};

/// The engine works on 16 kHz mono audio.
pub const SAMPLE_RATE: u32 = 16_000;

/// A file transcribed and diarized: speaker blocks plus the speakers' voice profiles.
#[allow(dead_code)] // VRT-17
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileTranscript {
    pub segments: Vec<Segment>,
    pub diarization: Diarization,
}

/// All transcription goes through this trait (AGENTS.md), so the helper-process engine can
/// be swapped for an in-process one (mobile). Audio is 16 kHz mono.
#[allow(dead_code)] // transcribe_file and speaker_profiles: VRT-17
pub trait Engine {
    /// A live caption chunk (4-8 s): words with timestamps, no speakers.
    fn transcribe_chunk(
        &self,
        samples: &[f32],
    ) -> impl Future<Output = Result<Transcript, String>> + Send;

    /// A recording or a rolling window (16 kHz mono WAV): speaker blocks with words.
    fn transcribe_file(
        &self,
        wav: &Path,
    ) -> impl Future<Output = Result<FileTranscript, String>> + Send;

    /// Words of a recording without speakers (the mic track is "Me"; 16 kHz mono WAV).
    fn transcribe_words(
        &self,
        wav: &Path,
    ) -> impl Future<Output = Result<Transcript, String>> + Send;

    /// Speaker turns and one embedding per usable speaker (16 kHz mono WAV).
    fn speaker_profiles(
        &self,
        wav: &Path,
    ) -> impl Future<Output = Result<Diarization, String>> + Send;
}

/// Write 16 kHz mono samples as a 16-bit WAV file for the engine.
pub fn write_wav(path: &Path, samples: &[f32]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for &s in samples {
        w.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
            .map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    /// "engine", "model" or "notice".
    pub item: &'static str,
    pub received: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HelperState {
    Stopped,
    Starting,
    Ready,
    Restarting,
    Failed,
}

/// Everything the UI shows about the engine (`engine_status`, event `engine:status`).
#[derive(Debug, Clone, Serialize)]
pub struct EngineStatus {
    /// A prebuilt engine exists for this platform.
    pub supported: bool,
    pub engine_installed: bool,
    pub model_installed: bool,
    /// Bytes still to download (engine layer, model, notice), resumed parts subtracted.
    pub download_size: u64,
    pub download: Option<DownloadProgress>,
    pub download_error: Option<String>,
    pub helper: HelperState,
    pub helper_info: Option<ReadyInfo>,
    pub helper_pid: Option<u32>,
    pub helper_error: Option<String>,
    /// Automatic restarts after a crash since the app started.
    pub restarts: u32,
}

/// Compressed size of the pinned engine layer (only used for the size shown up front).
const ENGINE_LAYER_BYTES: u64 = 14_447_296;
/// More crashes than this within CRASH_WINDOW stop the automatic restarts.
const MAX_CRASHES: usize = 3;
const CRASH_WINDOW: Duration = Duration::from_secs(60);

struct State {
    download: Option<DownloadProgress>,
    cancel: Option<Arc<AtomicBool>>,
    download_error: Option<String>,
    helper: Option<Arc<Helper>>,
    helper_state: HelperState,
    helper_error: Option<String>,
    restarts: u32,
    crashes: Vec<Instant>,
}

struct Shared {
    paths: AppPaths,
    notify: Box<dyn Fn(EngineStatus) + Send + Sync>,
    state: Mutex<State>,
    /// Serialises helper starts (manual, on demand and after a crash).
    start_lock: Mutex<()>,
    shutting_down: AtomicBool,
}

#[derive(Clone)]
pub struct EngineManager {
    shared: Arc<Shared>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl EngineManager {
    /// `notify` receives every status change (the app emits it as `engine:status`).
    pub fn new(paths: AppPaths, notify: impl Fn(EngineStatus) + Send + Sync + 'static) -> Self {
        Self {
            shared: Arc::new(Shared {
                paths,
                notify: Box::new(notify),
                state: Mutex::new(State {
                    download: None,
                    cancel: None,
                    download_error: None,
                    helper: None,
                    helper_state: HelperState::Stopped,
                    helper_error: None,
                    restarts: 0,
                    crashes: Vec::new(),
                }),
                start_lock: Mutex::new(()),
                shutting_down: AtomicBool::new(false),
            }),
        }
    }

    pub fn status(&self) -> EngineStatus {
        self.shared.status()
    }

    /// The model licences and attributions, once downloaded.
    pub fn notice(&self) -> Option<String> {
        std::fs::read_to_string(
            self.shared
                .paths
                .models_dir()
                .join(assets::NOTICE.file_name),
        )
        .ok()
    }

    /// Download whatever is missing, on a background thread. Progress goes to `notify`.
    pub fn start_download(&self) -> Result<(), String> {
        let image = assets::ENGINE.ok_or_else(unsupported)?;
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut st = lock(&self.shared.state);
            if st.download.is_some() {
                return Ok(());
            }
            st.download = Some(DownloadProgress {
                item: "engine",
                received: 0,
                total: 0,
            });
            st.cancel = Some(cancel.clone());
            st.download_error = None;
        }
        self.shared.changed();
        let shared = self.shared.clone();
        std::thread::Builder::new()
            .name("manent-download".into())
            .spawn(move || {
                let result = shared.download_all(&image, &cancel);
                {
                    let mut st = lock(&shared.state);
                    st.download = None;
                    st.cancel = None;
                    st.download_error = match result {
                        Ok(()) | Err(download::DownloadError::Cancelled) => None,
                        Err(e) => Some(e.to_string()),
                    };
                }
                shared.changed();
            })
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn cancel_download(&self) {
        if let Some(c) = &lock(&self.shared.state).cancel {
            c.store(true, Ordering::Relaxed);
        }
    }

    /// Start the helper if it isn't running (blocks while the model loads; call off the
    /// async runtime). A manual start also clears a "failed" state after repeated crashes.
    pub fn start_blocking(&self) -> Result<ReadyInfo, String> {
        lock(&self.shared.state).crashes.clear();
        Shared::start(&self.shared)
    }

    pub async fn start(&self) -> Result<ReadyInfo, String> {
        let this = self.clone();
        tauri::async_runtime::spawn_blocking(move || this.start_blocking())
            .await
            .map_err(|e| e.to_string())?
    }

    /// Run one helper request (starting the helper when needed).
    pub async fn request(&self, op: &str, wav: Option<&Path>) -> Result<Value, String> {
        let running = lock(&self.shared.state).helper.clone();
        let helper = match running {
            Some(h) => h,
            None => {
                let shared = self.shared.clone();
                tauri::async_runtime::spawn_blocking(move || Shared::start(&shared))
                    .await
                    .map_err(|e| e.to_string())??;
                lock(&self.shared.state)
                    .helper
                    .clone()
                    .ok_or("engine stopped")?
            }
        };
        helper.request(op, wav).await
    }

    /// Verify the installed engine and model (VRT-27). Damaged or missing files are removed,
    /// so the status shows them as not downloaded and Download fetches them again. Returns
    /// the names of the files that had to go (empty: everything is fine). Slow (hashes the
    /// 1.1 GB model): call off the async runtime.
    pub fn check_blocking(&self) -> Result<Vec<String>, String> {
        let image = assets::ENGINE.ok_or_else(unsupported)?;
        self.stop_helper();
        let mut damaged = Vec::new();
        let engine_dir = self.shared.paths.engine_dir();
        if download::engine_installed(&engine_dir, &image) {
            for name in download::damaged_engine_files(&engine_dir, &image) {
                // Without engine.json the whole engine downloads again.
                let _ = std::fs::remove_file(engine_dir.join("engine.json"));
                let _ = std::fs::remove_file(engine_dir.join(&name));
                damaged.push(name);
            }
        }
        for f in [&assets::MODEL, &assets::NOTICE] {
            let path = self.shared.paths.models_dir().join(f.file_name);
            if !path.is_file() {
                continue;
            }
            if !download::file_sha256(&path).is_ok_and(|h| h == f.sha256) {
                let _ = std::fs::remove_file(&path);
                damaged.push(f.file_name.to_string());
            }
        }
        self.shared.changed();
        Ok(damaged)
    }

    /// Delete the engine and the model, including partial downloads (frees ~1.1 GB).
    pub fn remove(&self) -> Result<(), String> {
        self.cancel_download();
        self.stop_helper();
        for dir in [
            self.shared.paths.engine_dir(),
            self.shared.paths.models_dir(),
        ] {
            if dir.exists() {
                std::fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
        }
        self.shared.changed();
        Ok(())
    }

    /// Stop the helper on purpose (no restart); the next request starts it again.
    fn stop_helper(&self) {
        let helper = {
            let mut st = lock(&self.shared.state);
            st.helper_state = HelperState::Stopped;
            st.helper_error = None;
            st.helper.take()
        };
        if let Some(h) = helper {
            h.stop();
        }
    }

    /// App exit: stop the helper and any download.
    pub fn shutdown(&self) {
        self.shared.shutting_down.store(true, Ordering::SeqCst);
        self.cancel_download();
        let helper = {
            let mut st = lock(&self.shared.state);
            st.helper_state = HelperState::Stopped;
            st.helper.take()
        };
        if let Some(h) = helper {
            h.stop();
        }
    }
}

impl Engine for EngineManager {
    async fn transcribe_chunk(&self, samples: &[f32]) -> Result<Transcript, String> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = self.shared.paths.tmp_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let wav = dir.join(format!(
            "chunk-{}-{}.wav",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        write_wav(&wav, samples)?;
        let doc = self.request("transcribe", Some(&wav)).await;
        let _ = std::fs::remove_file(&wav);
        transcript::parse_transcript(&doc?)
    }

    async fn transcribe_file(&self, wav: &Path) -> Result<FileTranscript, String> {
        // Words and speakers run in parallel (diarization is about a third of the time).
        let (words, speakers) = tokio::join!(
            self.request("transcribe", Some(wav)),
            self.request("diarize", Some(wav))
        );
        let words = transcript::parse_transcript(&words?)?;
        let diarization = transcript::parse_diarization(&speakers?)?;
        Ok(FileTranscript {
            segments: transcript::merge(&words.words, &diarization.turns),
            diarization,
        })
    }

    async fn transcribe_words(&self, wav: &Path) -> Result<Transcript, String> {
        transcript::parse_transcript(&self.request("transcribe", Some(wav)).await?)
    }

    async fn speaker_profiles(&self, wav: &Path) -> Result<Diarization, String> {
        transcript::parse_diarization(&self.request("diarize", Some(wav)).await?)
    }
}

fn unsupported() -> String {
    format!(
        "No prebuilt engine for {}-{} yet",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

impl Shared {
    fn model_path(&self) -> PathBuf {
        self.paths.models_dir().join(assets::MODEL.file_name)
    }

    fn engine_installed(&self) -> bool {
        assets::ENGINE.is_some_and(|img| download::engine_installed(&self.paths.engine_dir(), &img))
    }

    /// Model and notice present with the pinned size (hashes are checked when downloading).
    fn model_installed(&self) -> bool {
        [&assets::MODEL, &assets::NOTICE].iter().all(|f| {
            std::fs::metadata(self.paths.models_dir().join(f.file_name))
                .is_ok_and(|m| m.len() == f.size)
        })
    }

    fn status(&self) -> EngineStatus {
        let engine_installed = self.engine_installed();
        let model_installed = self.model_installed();
        let mut download_size = 0;
        if !engine_installed {
            download_size += ENGINE_LAYER_BYTES;
        }
        for f in [&assets::MODEL, &assets::NOTICE] {
            let dest = self.paths.models_dir().join(f.file_name);
            if !dest.is_file() {
                download_size += f.size.saturating_sub(download::partial_len(&dest));
            }
        }
        let st = lock(&self.state);
        EngineStatus {
            supported: assets::ENGINE.is_some(),
            engine_installed,
            model_installed,
            download_size,
            download: st.download.clone(),
            download_error: st.download_error.clone(),
            helper: st.helper_state,
            helper_info: st.helper.as_ref().map(|h| h.info.clone()),
            helper_pid: st.helper.as_ref().map(|h| h.pid),
            helper_error: st.helper_error.clone(),
            restarts: st.restarts,
        }
    }

    fn changed(&self) {
        (self.notify)(self.status());
    }

    fn download_all(
        &self,
        image: &assets::EngineImage,
        cancel: &AtomicBool,
    ) -> Result<(), download::DownloadError> {
        let agent = download::agent();
        // Progress events at most 4 per second.
        let mut last = Instant::now() - Duration::from_secs(1);
        let mut report = |item: &'static str, received: u64, total: u64| {
            lock(&self.state).download = Some(DownloadProgress {
                item,
                received,
                total,
            });
            if last.elapsed() >= Duration::from_millis(250) || received == total {
                last = Instant::now();
                self.changed();
            }
        };
        if !self.engine_installed() {
            download::install_engine(
                &agent,
                image,
                &self.paths.engine_dir(),
                cancel,
                &mut |r, t| report("engine", r, t),
            )?;
        }
        for (item, f) in [("model", &assets::MODEL), ("notice", &assets::NOTICE)] {
            let dest = self.paths.models_dir().join(f.file_name);
            if dest.is_file() {
                continue;
            }
            download::download(&agent, f.url, &dest, f.size, f.sha256, cancel, &mut |r| {
                report(item, r, f.size)
            })?;
        }
        Ok(())
    }

    fn start(this: &Arc<Self>) -> Result<ReadyInfo, String> {
        let _guard = lock(&this.start_lock);
        if let Some(h) = &lock(&this.state).helper {
            return Ok(h.info.clone());
        }
        let image = assets::ENGINE.ok_or_else(unsupported)?;
        let fail = |msg: String| {
            {
                let mut st = lock(&this.state);
                st.helper_state = HelperState::Failed;
                st.helper_error = Some(msg.clone());
            }
            this.changed();
            Err(msg)
        };
        if !this.engine_installed() || !this.model_installed() {
            return fail("The engine or model is not downloaded yet".into());
        }
        let Some(exe) = helper::helper_exe() else {
            return fail("manent-engine is missing next to the app".into());
        };
        {
            let mut st = lock(&this.state);
            if st.helper_state != HelperState::Restarting {
                st.helper_state = HelperState::Starting;
            }
            st.helper_error = None;
        }
        this.changed();
        let launch = Launch {
            exe,
            library: this.paths.engine_dir().join(image.library),
            model: this.model_path(),
        };
        let weak = Arc::downgrade(this);
        match Helper::spawn(&launch, move |reason| Shared::crashed(&weak, reason)) {
            Ok(helper) => {
                let info = helper.info.clone();
                {
                    let mut st = lock(&this.state);
                    st.helper = Some(helper);
                    st.helper_state = HelperState::Ready;
                }
                this.changed();
                Ok(info)
            }
            Err(e) => fail(e),
        }
    }

    /// The helper ended without being stopped: restart it, unless it keeps crashing.
    fn crashed(weak: &Weak<Self>, reason: String) {
        let Some(this) = weak.upgrade() else { return };
        if this.shutting_down.load(Ordering::SeqCst) {
            return;
        }
        eprintln!("[engine] helper stopped unexpectedly: {reason}");
        let give_up = {
            let mut st = lock(&this.state);
            st.helper = None;
            let now = Instant::now();
            st.crashes.retain(|t| now.duration_since(*t) < CRASH_WINDOW);
            st.crashes.push(now);
            if st.crashes.len() > MAX_CRASHES {
                st.helper_state = HelperState::Failed;
                st.helper_error = Some(format!(
                    "The engine crashed {} times in a minute; last: {reason}",
                    st.crashes.len()
                ));
                true
            } else {
                st.restarts += 1;
                st.helper_state = HelperState::Restarting;
                st.helper_error = Some(format!("The engine stopped ({reason}); restarting"));
                false
            }
        };
        this.changed();
        if give_up {
            return;
        }
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(1));
            if !this.shutting_down.load(Ordering::SeqCst) {
                let _ = Shared::start(&this);
            }
        });
    }
}

/// Integration tests through the real helper and model (VRT-15). They need the engine and the
/// model downloaded into a data folder (Settings > Transcription or scripts/smoke-engine.py):
/// `MANENT_TEST_ENGINE_DATA=<data dir> cargo test --workspace -- --ignored engine::`
#[cfg(test)]
mod engine_tests {
    use super::*;
    use std::time::Instant;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn engine() -> EngineManager {
        let data = std::env::var_os("MANENT_TEST_ENGINE_DATA")
            .expect("set MANENT_TEST_ENGINE_DATA to a data folder with the engine and model");
        let engine = EngineManager::new(AppPaths::new(data.into()), |_| {});
        let status = engine.status();
        assert!(
            status.engine_installed && status.model_installed,
            "engine or model missing"
        );
        engine
    }

    fn read_wav(path: &Path) -> Vec<f32> {
        hound::WavReader::open(path)
            .unwrap()
            .samples::<i16>()
            .map(|s| s.unwrap() as f32 / 32768.0)
            .collect()
    }

    fn norm(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric() && c != '\'')
            .filter(|w| !w.is_empty())
            .map(str::to_string)
            .collect()
    }

    fn wer(reference: &str, hypothesis: &str) -> f64 {
        let (r, h) = (norm(reference), norm(hypothesis));
        let mut d: Vec<usize> = (0..=h.len()).collect();
        for (i, rw) in r.iter().enumerate() {
            let mut prev = d[0];
            d[0] = i + 1;
            for (j, hw) in h.iter().enumerate() {
                let cur = (d[j + 1] + 1)
                    .min(d[j] + 1)
                    .min(prev + usize::from(rw != hw));
                prev = d[j + 1];
                d[j + 1] = cur;
            }
        }
        d[h.len()] as f64 / r.len().max(1) as f64
    }

    #[tokio::test]
    #[ignore = "needs the downloaded engine and model"]
    async fn transcribes_dutch_and_english_chunks() {
        let engine = engine();
        engine.start().await.unwrap();
        for (file, truth) in [
            (
                "en-fleurs-1652.wav",
                "the major religion in moldova is orthodox christian",
            ),
            (
                "nl-fleurs-1784.wav",
                "groenland had nog weinig inwoners volgens noorse sagen werd erik de rode uit ijsland verbannen wegens moord vond groenland op weg naar het westen en gaf het zijn naam",
            ),
        ] {
            let samples = read_wav(&fixture(file));
            let started = Instant::now();
            let t = engine.transcribe_chunk(&samples).await.unwrap();
            let took = started.elapsed();
            let audio_ms = (samples.len() as u64 * 1000 / SAMPLE_RATE as u64) as u32;
            let e = wer(truth, &t.text);
            println!(
                "{file}: {:.2} s for {:.1} s audio, WER {:.0}%: {}",
                took.as_secs_f64(),
                audio_ms as f64 / 1000.0,
                e * 100.0,
                t.text
            );
            assert!(e <= 0.2, "{file}: WER {e:.2}: {}", t.text);
            assert!(took.as_secs_f64() < 1.0, "{file}: {took:?}");
            assert!(!t.words.is_empty());
            assert!(t.words.windows(2).all(|w| w[0].start_ms <= w[1].start_ms));
            assert!(
                t.words
                    .iter()
                    .all(|w| w.start_ms <= w.end_ms && w.end_ms <= audio_ms + 100)
            );
        }
    }

    #[tokio::test]
    #[ignore = "needs the downloaded engine and model"]
    async fn transcribes_a_file_with_speakers_and_profiles() {
        let engine = engine();
        let wav = fixture("en-two-speakers.wav");
        let started = Instant::now();
        let file = engine.transcribe_file(&wav).await.unwrap();
        println!("transcribe_file: {:.1} s", started.elapsed().as_secs_f64());
        for s in &file.segments {
            println!(
                "  [{:>5}-{:>5}] {:?} {}",
                s.start_ms, s.end_ms, s.speaker, s.text
            );
        }
        // The fixture alternates two voices: turns 1 and 3 are one person, 2 and 4 the other.
        let truth: Value = serde_json::from_str(
            &std::fs::read_to_string(fixture("en-two-speakers.truth.json")).unwrap(),
        )
        .unwrap();
        let mut per_turn = Vec::new();
        let mut all_text = String::new();
        for t in truth["segments"].as_array().unwrap() {
            let (a, b) = (
                (t["start"].as_f64().unwrap() * 1000.0) as u32,
                (t["end"].as_f64().unwrap() * 1000.0) as u32,
            );
            let words: Vec<_> = file
                .segments
                .iter()
                .flat_map(|s| s.words.iter().map(move |w| (s.speaker, w)))
                .filter(|(_, w)| w.start_ms >= a.saturating_sub(300) && w.end_ms <= b + 300)
                .collect();
            let mut counts = std::collections::HashMap::new();
            for (spk, _) in &words {
                *counts.entry(*spk).or_insert(0) += 1;
            }
            per_turn.push(
                counts
                    .into_iter()
                    .max_by_key(|(_, n)| *n)
                    .and_then(|(s, _)| s),
            );
            all_text.push_str(t["text"].as_str().unwrap());
            all_text.push(' ');
        }
        println!("speaker per turn: {per_turn:?}");
        assert!(per_turn.iter().all(Option::is_some));
        assert_eq!(per_turn[0], per_turn[2]);
        assert_eq!(per_turn[1], per_turn[3]);
        assert_ne!(per_turn[0], per_turn[1]);
        let hyp: Vec<_> = file.segments.iter().map(|s| s.text.as_str()).collect();
        let e = wer(&all_text, &hyp.join(" "));
        println!("WER {:.1}%", e * 100.0);
        assert!(e <= 0.15, "WER {e:.2}");

        let d = engine.speaker_profiles(&wav).await.unwrap();
        assert!(d.encoder_id.starts_with("sha256:"), "{}", d.encoder_id);
        let usable: Vec<_> = d
            .profiles
            .iter()
            .filter(|p| p.embedding.is_some())
            .collect();
        assert_eq!(
            usable.len(),
            2,
            "{:?}",
            d.profiles
                .iter()
                .map(|p| (p.speaker, p.clean_seconds))
                .collect::<Vec<_>>()
        );
        for p in usable {
            let e = p.embedding.as_ref().unwrap();
            let n: f32 = e.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert_eq!(e.len(), 256);
            assert!(
                (n - 1.0).abs() < 0.01 && p.clean_seconds > 5.0,
                "norm {n}, clean {}",
                p.clean_seconds
            );
        }
    }
}
