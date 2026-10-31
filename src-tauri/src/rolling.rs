//! The rolling and final passes (VRT-17, doc-2 §1.3). While recording, a rolling pass runs
//! about every 45 s over the newest audio of every track (from the caption workers'
//! history), with 30 s of context before it: the system track is transcribed and diarized,
//! the mic track transcribed as "Me", bleed removed, and the result replaces the captions of
//! that stretch. After the recording the final pass does the same over the whole meeting at
//! once, so speaker labels are consistent everywhere.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::audio::SourceKind;
use crate::engine::transcript::{Diarization, SpeakerTurn, Word};
use crate::engine::{Engine, SAMPLE_RATE, write_wav};
use crate::live::{SharedHistory, TrackRef};
use crate::passes::{self, Labelled, RollingLabels};

const RATE: u64 = SAMPLE_RATE as u64;
/// Time between rolling passes.
const INTERVAL: Duration = Duration::from_secs(45);
/// A pass needs at least this much new audio.
const MIN_NEW: u64 = 20 * RATE;
/// Audio before the new stretch that is transcribed and diarized along for context.
const CONTEXT: u64 = 30 * RATE;
/// The newest second is left for the next pass (a word may still be running).
const MARGIN_MS: u32 = 1000;

/// A track the passes work on.
pub struct PassTrack {
    pub track: TrackRef,
    pub history: SharedHistory,
}

/// One pass's result: blocks per track id, replacing segments that start in `[from_ms, to_ms)`.
#[derive(Debug, Clone)]
pub struct PassResult {
    pub from_ms: u32,
    pub to_ms: u32,
    pub blocks: Vec<(i64, Labelled)>,
}

pub struct Rolling {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

impl Rolling {
    /// Stop after the pass that may be running; returns once the thread ended.
    pub fn stop(self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = self.handle.join();
    }
}

fn shift(words: &mut [Word], ms: u32) {
    for w in words {
        w.start_ms += ms;
        w.end_ms += ms;
    }
}

fn shift_diarization(d: &mut Diarization, ms: u32) {
    for t in &mut d.turns {
        t.start_ms += ms;
        t.end_ms += ms;
    }
}

/// The end of the stretch to keep: about `end_ms - MARGIN_MS`, moved back to the start of
/// any word it would cut, so no word is split between passes.
pub fn safe_end(words: &[&Word], end_ms: u32) -> u32 {
    let mut to = end_ms.saturating_sub(MARGIN_MS);
    loop {
        match words.iter().find(|w| w.start_ms < to && w.end_ms > to) {
            Some(w) => to = w.start_ms,
            None => return to,
        }
    }
}

/// Transcribe the given 16 kHz audio of each track: system tracks with speakers, the mic as
/// words. Returns (track, words, diarization) with times relative to the audio start.
async fn transcribe_tracks<E: Engine + Clone + Send + Sync + 'static>(
    engine: &E,
    audio: Vec<(TrackRef, PathBuf)>,
) -> Result<Vec<(TrackRef, Vec<Word>, Option<Diarization>)>, String> {
    // One task per track, so the helper runs them on its parallel backends.
    let tasks: Vec<_> = audio
        .into_iter()
        .map(|(track, wav)| {
            let engine = engine.clone();
            tauri::async_runtime::spawn(async move {
                match track.kind {
                    SourceKind::Loopback => engine.transcribe_file(&wav).await.map(|f| {
                        let words = f.segments.into_iter().flat_map(|s| s.words).collect();
                        (track, words, Some(f.diarization))
                    }),
                    SourceKind::Mic => engine
                        .transcribe_words(&wav)
                        .await
                        .map(|t| (track, t.words, None)),
                }
            })
        })
        .collect();
    let mut out = Vec::new();
    for task in tasks {
        out.push(task.await.map_err(|e| e.to_string())??);
    }
    Ok(out)
}

/// Bleed removal and labelling of one pass's words (all times in meeting ms).
fn label_blocks(
    tracks: &[(TrackRef, Vec<Word>, Option<Diarization>)],
    mut labels_for: impl FnMut(&Diarization, &[Word]) -> std::collections::HashMap<u32, String>,
) -> Vec<(i64, Labelled)> {
    let system_words: Vec<Word> = tracks
        .iter()
        .filter(|(_, _, d)| d.is_some())
        .flat_map(|(_, w, _)| w.iter().cloned())
        .collect();
    let mut blocks = Vec::new();
    for (track, words, diar) in tracks {
        match diar {
            Some(d) => {
                let labels = labels_for(d, words);
                let turns: Vec<SpeakerTurn> = d.turns.clone();
                blocks.extend(
                    passes::system_blocks(words, &turns, &labels)
                        .into_iter()
                        .map(|b| (track.track_id, b)),
                );
            }
            None => {
                let kept = passes::remove_bleed(words, &system_words);
                blocks.extend(
                    passes::me_blocks(&kept)
                        .into_iter()
                        .map(|b| (track.track_id, b)),
                );
            }
        }
    }
    blocks.sort_by_key(|(_, b)| b.start_ms);
    blocks
}

/// Start the rolling passes of a recording. `on_pass` stores and shows each result;
/// `on_error` reports a failed pass (the next one tries again from the same point).
pub fn spawn<E: Engine + Clone + Send + Sync + 'static>(
    tracks: Vec<PassTrack>,
    engine: E,
    tmp: PathBuf,
    on_pass: impl Fn(PassResult) + Send + 'static,
    on_error: impl Fn(String) + Send + 'static,
) -> std::io::Result<Rolling> {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let handle = std::thread::Builder::new()
        .name("veritas-rolling".into())
        .spawn(move || {
            let mut labels = RollingLabels::default();
            let mut from: u64 = 0;
            let mut last = Instant::now();
            while !flag.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(250));
                if last.elapsed() < INTERVAL {
                    continue;
                }
                last = Instant::now();
                match tauri::async_runtime::block_on(rolling_pass(
                    &tracks,
                    &engine,
                    &tmp,
                    from,
                    &mut labels,
                )) {
                    Ok(Some((result, next))) => {
                        from = next;
                        on_pass(result);
                    }
                    Ok(None) => {}
                    Err(e) => on_error(e),
                }
            }
        })?;
    Ok(Rolling { stop, handle })
}

/// One rolling pass from track sample `from`. Returns the result and where the next starts.
async fn rolling_pass<E: Engine + Clone + Send + Sync + 'static>(
    tracks: &[PassTrack],
    engine: &E,
    tmp: &Path,
    from: u64,
    labels: &mut RollingLabels,
) -> Result<Option<(PassResult, u64)>, String> {
    let lock = |h: &SharedHistory| {
        let g = h.lock().unwrap_or_else(|e| e.into_inner());
        (g.start(), g.end())
    };
    let end = tracks.iter().map(|t| lock(&t.history).1).min().unwrap_or(0);
    if end < from + MIN_NEW {
        return Ok(None);
    }
    let ctx = tracks
        .iter()
        .map(|t| lock(&t.history).0)
        .max()
        .unwrap_or(0)
        .max(from.saturating_sub(CONTEXT));
    std::fs::create_dir_all(tmp).map_err(|e| e.to_string())?;
    let mut audio = Vec::new();
    for t in tracks {
        let samples = t
            .history
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .range(ctx, end);
        let wav = tmp.join(format!("rolling-{}.wav", t.track.track_id));
        write_wav(&wav, &samples)?;
        audio.push((t.track.clone(), wav));
    }
    let files: Vec<PathBuf> = audio.iter().map(|(_, w)| w.clone()).collect();
    let result = transcribe_tracks(engine, audio).await;
    for f in files {
        let _ = std::fs::remove_file(f);
    }
    let mut results = result?;

    let ms = |samples: u64| (samples * 1000 / RATE) as u32;
    let (ctx_ms, from_ms, end_ms) = (ms(ctx), ms(from), ms(end));
    for (_, words, diar) in &mut results {
        shift(words, ctx_ms);
        if let Some(d) = diar {
            shift_diarization(d, ctx_ms);
        }
    }
    let all: Vec<&Word> = results.iter().flat_map(|(_, w, _)| w.iter()).collect();
    let to_ms = safe_end(&all, end_ms);
    if to_ms <= from_ms {
        return Ok(None);
    }
    // Keep the words of the new stretch; the context only helps the model and the labels.
    for (_, words, _) in &mut results {
        words.retain(|w| w.start_ms >= from_ms && w.start_ms < to_ms);
    }
    let blocks = label_blocks(&results, |d, kept| labels.assign(d, kept));
    Ok(Some((
        PassResult {
            from_ms,
            to_ms,
            blocks,
        },
        to_ms as u64 * RATE / 1000,
    )))
}

/// The final pass over whole tracks (16 kHz mono audio per track).
pub async fn final_pass<E: Engine + Clone + Send + Sync + 'static>(
    engine: &E,
    tracks: Vec<(TrackRef, Vec<f32>)>,
    tmp: &Path,
) -> Result<Vec<(i64, Labelled)>, String> {
    std::fs::create_dir_all(tmp).map_err(|e| e.to_string())?;
    let mut audio = Vec::new();
    for (track, samples) in &tracks {
        let wav = tmp.join(format!("final-{}.wav", track.track_id));
        write_wav(&wav, samples)?;
        audio.push((track.clone(), wav));
    }
    let files: Vec<PathBuf> = audio.iter().map(|(_, w)| w.clone()).collect();
    let result = transcribe_tracks(engine, audio).await;
    for f in files {
        let _ = std::fs::remove_file(f);
    }
    let results = result?;
    Ok(label_blocks(&results, |d, words| {
        passes::number_by_appearance(words, &d.turns)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(start_ms: u32, end_ms: u32) -> Word {
        Word {
            text: "x".into(),
            start_ms,
            end_ms,
            conf: 1.0,
        }
    }

    #[test]
    fn the_kept_stretch_never_ends_inside_a_word() {
        let words = [w(0, 500), w(8800, 9300), w(9300, 9600)];
        let refs: Vec<&Word> = words.iter().collect();
        // 10 s of audio: the margin puts the end at 9 s, inside the word 8.8-9.3 s.
        assert_eq!(safe_end(&refs, 10_000), 8800);
        assert_eq!(safe_end(&refs[..1], 10_000), 9000);
        // A chain of touching words moves the end back to the first one.
        let chain = [w(8000, 8600), w(8500, 9100)];
        let refs: Vec<&Word> = chain.iter().collect();
        assert_eq!(safe_end(&refs, 10_000), 8000);
    }
}

/// The final pass through the real engine (VERITAS_TEST_ENGINE_DATA, see engine::engine_tests):
/// the mic hears the speakers (bleed) and the user says something in a pause.
#[cfg(test)]
mod engine_tests {
    use super::*;
    use crate::engine::EngineManager;
    use crate::paths::AppPaths;

    fn fixture(name: &str) -> Vec<f32> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name);
        hound::WavReader::open(path)
            .unwrap()
            .samples::<i16>()
            .map(|s| s.unwrap() as f32 / 32768.0)
            .collect()
    }

    fn track(track_id: i64, kind: SourceKind) -> TrackRef {
        TrackRef {
            meeting_id: 1,
            track_id,
            source_id: format!("{kind:?}"),
            kind,
        }
    }

    #[tokio::test]
    #[ignore = "needs the downloaded engine and model"]
    async fn final_pass_labels_speakers_and_keeps_only_the_users_own_words() {
        let data = std::env::var_os("VERITAS_TEST_ENGINE_DATA").expect("VERITAS_TEST_ENGINE_DATA");
        let paths = AppPaths { data: data.into() };
        let engine = EngineManager::new(paths.clone(), |_| {});
        // System: two remote speakers, then 13 s of silence.
        let mut system = fixture("en-two-speakers.wav");
        let remote_len = system.len();
        system.resize(remote_len + 13 * RATE as usize, 0.0);
        // Mic: the speakers at 30%, 40 ms late, plus the user (Dutch) in the silence.
        let delay = 40 * RATE as usize / 1000;
        let mut mic = vec![0.0f32; system.len()];
        for (i, x) in system.iter().enumerate().take(system.len() - delay) {
            mic[i + delay] = 0.3 * x;
        }
        let user = fixture("nl-fleurs-1784.wav");
        let at = remote_len + RATE as usize;
        for (i, x) in user.iter().enumerate() {
            mic[at + i] += x;
        }
        let tmp = crate::test_util::temp_dir();
        let blocks = final_pass(
            &engine,
            vec![
                (track(1, SourceKind::Mic), mic),
                (track(2, SourceKind::Loopback), system),
            ],
            &tmp,
        )
        .await
        .unwrap();
        for (id, b) in &blocks {
            println!(
                "track {id} {:>5}-{:>5} {:>2} {}",
                b.start_ms, b.end_ms, b.label, b.text
            );
        }
        let me: Vec<_> = blocks
            .iter()
            .filter(|(_, b)| b.label == passes::ME)
            .collect();
        let me_text: String = me.iter().map(|(_, b)| b.text.to_lowercase()).collect();
        assert!(
            me_text.contains("groenland"),
            "the user's words are kept: {me_text}"
        );
        assert!(
            me.iter()
                .all(|(_, b)| b.start_ms as usize >= remote_len * 1000 / RATE as usize),
            "nothing of the remote speakers is attributed to 'Me'"
        );
        let remote: std::collections::HashSet<_> = blocks
            .iter()
            .filter(|(id, _)| *id == 2)
            .map(|(_, b)| b.label.clone())
            .collect();
        assert_eq!(remote.len(), 2, "two remote speakers: {remote:?}");
    }
}
