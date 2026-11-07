//! The rolling and final passes (VRT-17, doc-2 §1.3). While recording, a rolling pass runs
//! about every 45 s over the newest audio of every track (from the caption workers'
//! history), with 30 s of context before it: the system track is transcribed and diarized,
//! the mic track transcribed as "Me" (in a room, VRT-47: diarized like the system track), bleed
//! removed, and the result replaces the captions of
//! that stretch. After the recording the final pass does the same over the whole meeting at
//! once, so speaker labels are consistent everywhere.

use std::collections::HashMap;
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
/// Time between rolling passes unless Settings says otherwise (VRT-27).
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(45);
/// A pass needs at least this much new audio.
const MIN_NEW: u64 = 20 * RATE;
/// Audio before the new stretch that is transcribed and diarized along for context.
const CONTEXT: u64 = 30 * RATE;
/// The newest second is left for the next pass (a word may still be running).
const MARGIN_MS: u32 = 1000;
/// A track with less audio than this has nothing to transcribe (a source that never delivered,
/// VRT-12); the engine refuses to diarize no audio, which would fail the other tracks too.
const MIN_TRACK: usize = SAMPLE_RATE as usize;
/// A track this far behind the others has stopped delivering; passes go on without it.
const STALLED: u64 = 10 * RATE;

/// How the rolling passes of a recording run.
pub struct PassOptions {
    /// Time between passes (Settings, VRT-27).
    pub interval: Duration,
    /// In a room the mic is diarized too; switchable while recording (VRT-47).
    pub in_room: Arc<AtomicBool>,
}

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
    /// The voice of every label in this pass (for naming and matching, VRT-18).
    pub voices: Vec<LabelVoice>,
}

/// A speaker label's voice in a pass: its cleanest embedding.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelVoice {
    pub label: String,
    pub embedding: Option<Vec<f32>>,
    pub clean_seconds: f32,
    pub encoder_id: String,
}

/// The voices of the labelled slots (several slots can share a label: keep the cleanest).
fn label_voices(d: &Diarization, labels: &HashMap<u32, String>) -> Vec<LabelVoice> {
    let mut out: Vec<LabelVoice> = Vec::new();
    for p in &d.profiles {
        let Some(label) = labels.get(&p.speaker) else {
            continue;
        };
        let voice = LabelVoice {
            label: label.clone(),
            embedding: p.embedding.clone(),
            clean_seconds: p.clean_seconds,
            encoder_id: d.encoder_id.clone(),
        };
        match out.iter_mut().find(|v| &v.label == label) {
            Some(v) if v.embedding.is_some() && v.clean_seconds >= voice.clean_seconds => {}
            Some(v) => *v = voice,
            None => out.push(voice),
        }
    }
    out
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
/// words (in a room with speakers too). Returns (track, words, diarization) with times
/// relative to the audio start.
async fn transcribe_tracks<E: Engine + Clone + Send + Sync + 'static>(
    engine: &E,
    audio: Vec<(TrackRef, PathBuf)>,
    in_room: bool,
) -> Result<Vec<(TrackRef, Vec<Word>, Option<Diarization>)>, String> {
    // One task per track, so the helper runs them on its parallel backends.
    let tasks: Vec<_> = audio
        .into_iter()
        .map(|(track, wav)| {
            let engine = engine.clone();
            tauri::async_runtime::spawn(async move {
                if track.kind == SourceKind::Mic && !in_room {
                    engine
                        .transcribe_words(&wav)
                        .await
                        .map(|t| (track, t.words, None))
                } else {
                    engine.transcribe_file(&wav).await.map(|f| {
                        let words = f.segments.into_iter().flat_map(|s| s.words).collect();
                        (track, words, Some(f.diarization))
                    })
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

/// A diarized track's words to label: (track id, its diarization, its words after bleed removal).
type ToLabel<'a> = (i64, &'a Diarization, &'a [Word]);

/// Bleed removal and labelling of one pass's words (all times in meeting ms). `labels_for`
/// gets every diarized track at once (their numbering is shared) and returns one label map
/// per track, in that order.
fn label_blocks(
    tracks: &[(TrackRef, Vec<Word>, Option<Diarization>)],
    labels_for: impl FnOnce(&[ToLabel]) -> Vec<HashMap<u32, String>>,
) -> (Vec<(i64, Labelled)>, Vec<LabelVoice>) {
    let system_words: Vec<Word> = tracks
        .iter()
        .filter(|(t, _, _)| t.kind == SourceKind::Loopback)
        .flat_map(|(_, w, _)| w.iter().cloned())
        .collect();
    // The mic also hears the speakers: those words belong to the system track.
    let words: Vec<Vec<Word>> = tracks
        .iter()
        .map(|(t, w, _)| match t.kind {
            SourceKind::Mic => passes::remove_bleed(w, &system_words),
            SourceKind::Loopback => w.clone(),
        })
        .collect();
    let diarized: Vec<ToLabel> = tracks
        .iter()
        .zip(&words)
        .filter_map(|((t, _, d), w)| Some((t.track_id, d.as_ref()?, w.as_slice())))
        .collect();
    let mut labels = labels_for(&diarized).into_iter();
    let mut blocks = Vec::new();
    let mut voices = Vec::new();
    for ((track, _, diar), words) in tracks.iter().zip(&words) {
        match diar {
            Some(d) => {
                let labels = labels.next().unwrap_or_default();
                voices.extend(label_voices(d, &labels));
                blocks.extend(
                    passes::system_blocks(words, &d.turns, &labels)
                        .into_iter()
                        .map(|b| (track.track_id, b)),
                );
            }
            None => blocks.extend(
                passes::me_blocks(words)
                    .into_iter()
                    .map(|b| (track.track_id, b)),
            ),
        }
    }
    blocks.sort_by_key(|(_, b)| b.start_ms);
    (blocks, voices)
}

/// Start the rolling passes of a recording. `on_pass` stores and shows each result;
/// `on_error` reports a failed pass (the next one tries again from the same point).
pub fn spawn<E: Engine + Clone + Send + Sync + 'static>(
    tracks: Vec<PassTrack>,
    engine: E,
    tmp: PathBuf,
    options: PassOptions,
    on_pass: impl Fn(PassResult) + Send + 'static,
    on_error: impl Fn(String) + Send + 'static,
) -> std::io::Result<Rolling> {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let handle = std::thread::Builder::new()
        .name("manent-rolling".into())
        .spawn(move || {
            let mut labels = RollingLabels::default();
            let mut from: u64 = 0;
            let mut last = Instant::now();
            while !flag.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(250));
                if last.elapsed() < options.interval {
                    continue;
                }
                last = Instant::now();
                match tauri::async_runtime::block_on(rolling_pass(
                    &tracks,
                    &engine,
                    &tmp,
                    from,
                    options.in_room.load(Ordering::SeqCst),
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
    in_room: bool,
    labels: &mut RollingLabels,
) -> Result<Option<(PassResult, u64)>, String> {
    let lock = |h: &SharedHistory| {
        let g = h.lock().unwrap_or_else(|e| e.into_inner());
        (g.start(), g.end())
    };
    // Tracks deliver a little apart: the pass ends where all of them have audio, but a track
    // that stopped delivering (or never did) would hold every pass back, so it sits this out.
    let newest = tracks.iter().map(|t| lock(&t.history).1).max().unwrap_or(0);
    let tracks: Vec<&PassTrack> = tracks
        .iter()
        .filter(|t| lock(&t.history).1 + STALLED >= newest)
        .collect();
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
        if samples.len() < MIN_TRACK {
            continue;
        }
        let wav = tmp.join(format!("rolling-{}.wav", t.track.track_id));
        write_wav(&wav, &samples)?;
        audio.push((t.track.clone(), wav));
    }
    let files: Vec<PathBuf> = audio.iter().map(|(_, w)| w.clone()).collect();
    let result = transcribe_tracks(engine, audio, in_room).await;
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
    let (blocks, voices) = label_blocks(&results, |tracks| {
        tracks
            .iter()
            .map(|(track, d, kept)| labels.assign(*track, d, kept))
            .collect()
    });
    Ok(Some((
        PassResult {
            from_ms,
            to_ms,
            blocks,
            voices,
        },
        to_ms as u64 * RATE / 1000,
    )))
}

/// The final pass over whole tracks (16 kHz mono audio per track); `in_room` diarizes the mic.
pub async fn final_pass<E: Engine + Clone + Send + Sync + 'static>(
    engine: &E,
    tracks: Vec<(TrackRef, Vec<f32>)>,
    tmp: &Path,
    in_room: bool,
) -> Result<(Vec<(i64, Labelled)>, Vec<LabelVoice>), String> {
    std::fs::create_dir_all(tmp).map_err(|e| e.to_string())?;
    let mut audio = Vec::new();
    for (track, samples) in tracks.iter().filter(|(_, s)| s.len() >= MIN_TRACK) {
        let wav = tmp.join(format!("final-{}.wav", track.track_id));
        write_wav(&wav, samples)?;
        audio.push((track.clone(), wav));
    }
    if audio.is_empty() {
        return Err("nothing was recorded".into());
    }
    let files: Vec<PathBuf> = audio.iter().map(|(_, w)| w.clone()).collect();
    let result = transcribe_tracks(engine, audio, in_room).await;
    for f in files {
        let _ = std::fs::remove_file(f);
    }
    let results = result?;
    Ok(label_blocks(&results, |tracks| {
        let tracks: Vec<(&[Word], &[SpeakerTurn])> = tracks
            .iter()
            .map(|(_, d, words)| (*words, d.turns.as_slice()))
            .collect();
        passes::number_by_appearance(&tracks)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{FileTranscript, Segment, Transcript};

    fn w(start_ms: u32, end_ms: u32) -> Word {
        Word {
            text: "x".into(),
            start_ms,
            end_ms,
            conf: 1.0,
        }
    }

    fn said(text: &str, start_ms: u32, end_ms: u32) -> Word {
        Word {
            text: text.into(),
            start_ms,
            end_ms,
            conf: 1.0,
        }
    }

    fn tref(track_id: i64, kind: SourceKind) -> TrackRef {
        TrackRef {
            meeting_id: 1,
            track_id,
            source_id: format!("{kind:?}"),
            kind,
        }
    }

    fn diarized(turns: Vec<SpeakerTurn>) -> Diarization {
        Diarization {
            turns,
            profiles: Vec::new(),
            encoder_id: "sha256:x".into(),
        }
    }

    fn turn(speaker: u32, start_ms: u32, end_ms: u32) -> SpeakerTurn {
        SpeakerTurn {
            speaker,
            start_ms,
            end_ms,
        }
    }

    fn final_labels(tracks: &[ToLabel]) -> Vec<HashMap<u32, String>> {
        let tracks: Vec<(&[Word], &[SpeakerTurn])> = tracks
            .iter()
            .map(|(_, d, w)| (*w, d.turns.as_slice()))
            .collect();
        passes::number_by_appearance(&tracks)
    }

    #[test]
    fn in_a_room_the_mic_has_speakers_and_still_loses_its_bleed() {
        // The mic hears two people in the room and, at 5 s, the remote person through the
        // speakers; the system track has that remote person.
        let mic = vec![
            said("Goedemorgen", 0, 800),
            said("allemaal.", 800, 1500),
            said("Zullen", 3000, 3400),
            said("we", 3400, 3600),
            said("beginnen?", 3600, 4200),
            said("hello", 5100, 5600),
            said("there", 5600, 6000),
        ];
        let mic_diar = diarized(vec![
            turn(0, 0, 1600),
            turn(1, 2900, 4300),
            turn(0, 5000, 6100),
        ]);
        let system = vec![said("Hello", 5000, 5500), said("there.", 5500, 5900)];
        let sys_diar = diarized(vec![turn(0, 5000, 6000)]);
        let tracks = vec![
            (tref(1, SourceKind::Mic), mic, Some(mic_diar)),
            (tref(2, SourceKind::Loopback), system, Some(sys_diar)),
        ];
        let (blocks, _) = label_blocks(&tracks, final_labels);
        let got: Vec<(i64, &str, &str)> = blocks
            .iter()
            .map(|(t, b)| (*t, b.label.as_str(), b.text.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                (1, "1", "Goedemorgen allemaal."),
                (1, "2", "Zullen we beginnen?"),
                (2, "3", "Hello there."),
            ]
        );
    }

    #[test]
    fn in_a_call_the_mic_is_me() {
        let tracks = vec![
            (tref(1, SourceKind::Mic), vec![said("Hoi", 0, 500)], None),
            (
                tref(2, SourceKind::Loopback),
                vec![said("Hi", 1000, 1500)],
                Some(diarized(vec![turn(4, 900, 1600)])),
            ),
        ];
        let (blocks, _) = label_blocks(&tracks, final_labels);
        let labels: Vec<&str> = blocks.iter().map(|(_, b)| b.label.as_str()).collect();
        assert_eq!(labels, [passes::ME, "1"]);
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

    /// Hears one word per file, "me" or "them", and like the real helper refuses no audio.
    #[derive(Clone)]
    struct OneWordEngine;

    impl OneWordEngine {
        fn word(wav: &Path, text: &str) -> Result<Vec<Word>, String> {
            let reader = hound::WavReader::open(wav).map_err(|e| e.to_string())?;
            if reader.duration() == 0 {
                return Err("diarize: no audio".into());
            }
            Ok(vec![said(text, 1000, 1500)])
        }
    }

    impl Engine for OneWordEngine {
        async fn transcribe_chunk(&self, _: &[f32]) -> Result<Transcript, String> {
            unreachable!()
        }
        async fn transcribe_file(&self, wav: &Path) -> Result<FileTranscript, String> {
            let words = Self::word(wav, "them")?;
            Ok(FileTranscript {
                segments: vec![Segment {
                    speaker: Some(0),
                    start_ms: 1000,
                    end_ms: 1500,
                    text: "them".into(),
                    words,
                }],
                diarization: diarized(vec![SpeakerTurn {
                    speaker: 0,
                    start_ms: 0,
                    end_ms: 5000,
                }]),
            })
        }
        async fn transcribe_words(&self, wav: &Path) -> Result<Transcript, String> {
            Ok(Transcript {
                text: "me".into(),
                words: Self::word(wav, "me")?,
            })
        }
        async fn speaker_profiles(&self, _: &Path) -> Result<Diarization, String> {
            unreachable!()
        }
    }

    fn seconds(s: f32) -> Vec<f32> {
        vec![0.1; (s * SAMPLE_RATE as f32) as usize]
    }

    fn tracks_with(lines: &[Vec<(i64, Labelled)>]) -> Vec<i64> {
        let mut ids: Vec<i64> = lines.iter().flatten().map(|(id, _)| *id).collect();
        ids.sort();
        ids.dedup();
        ids
    }

    #[tokio::test]
    async fn the_final_pass_skips_a_track_that_recorded_nothing() {
        let tmp = crate::test_util::temp_dir();
        let (blocks, _) = final_pass(
            &OneWordEngine,
            vec![
                (tref(1, SourceKind::Mic), seconds(5.0)),
                (tref(2, SourceKind::Loopback), Vec::new()),
            ],
            &tmp,
            false,
        )
        .await
        .unwrap();
        assert_eq!(tracks_with(&[blocks]), [1], "the mic is transcribed");

        let none = final_pass(
            &OneWordEngine,
            vec![(tref(2, SourceKind::Loopback), Vec::new())],
            &tmp,
            false,
        )
        .await;
        assert_eq!(none.unwrap_err(), "nothing was recorded");
    }

    #[tokio::test]
    async fn rolling_passes_go_on_without_a_track_that_stopped_delivering() {
        let history = |s: f32| {
            let h = SharedHistory::default();
            h.lock().unwrap().push(&seconds(s));
            h
        };
        let pass = |system: f32| {
            let tracks = vec![
                PassTrack {
                    track: tref(1, SourceKind::Mic),
                    history: history(30.0),
                },
                PassTrack {
                    track: tref(2, SourceKind::Loopback),
                    history: history(system),
                },
            ];
            async move {
                let tmp = crate::test_util::temp_dir();
                let mut labels = RollingLabels::default();
                rolling_pass(&tracks, &OneWordEngine, &tmp, 0, false, &mut labels)
                    .await
                    .unwrap()
                    .map(|(r, _)| r.blocks)
                    .unwrap_or_default()
            }
        };
        assert_eq!(tracks_with(&[pass(0.0).await]), [1], "never delivered");
        assert_eq!(tracks_with(&[pass(5.0).await]), [1], "stopped at 5 s");
        assert_eq!(
            tracks_with(&[pass(29.5).await]),
            [1, 2],
            "a little behind is still in"
        );
    }
}

/// The final pass through the real engine (MANENT_TEST_ENGINE_DATA, see engine::engine_tests):
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
        let data = std::env::var_os("MANENT_TEST_ENGINE_DATA").expect("MANENT_TEST_ENGINE_DATA");
        let paths = AppPaths::new(data.into());
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
        let (blocks, _) = final_pass(
            &engine,
            vec![
                (track(1, SourceKind::Mic), mic),
                (track(2, SourceKind::Loopback), system),
            ],
            &tmp,
            false,
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

    #[tokio::test]
    #[ignore = "needs the downloaded engine and model"]
    async fn in_a_room_the_people_on_the_mic_are_told_apart() {
        let data = std::env::var_os("MANENT_TEST_ENGINE_DATA").expect("MANENT_TEST_ENGINE_DATA");
        let paths = AppPaths::new(data.into());
        let engine = EngineManager::new(paths.clone(), |_| {});
        // The mic hears two people in the room, then a third (Dutch); the system is quiet.
        let mut mic = fixture("en-two-speakers.wav");
        mic.extend(std::iter::repeat_n(0.0, RATE as usize));
        mic.extend(fixture("nl-fleurs-1784.wav"));
        let system = vec![0.0f32; mic.len()];
        let tmp = crate::test_util::temp_dir();
        let (blocks, voices) = final_pass(
            &engine,
            vec![
                (track(1, SourceKind::Mic), mic),
                (track(2, SourceKind::Loopback), system),
            ],
            &tmp,
            true,
        )
        .await
        .unwrap();
        for (id, b) in &blocks {
            println!(
                "track {id} {:>5}-{:>5} {:>2} {}",
                b.start_ms, b.end_ms, b.label, b.text
            );
        }
        assert!(
            blocks.iter().all(|(_, b)| b.label != passes::ME),
            "in a room nobody is 'Me' by default"
        );
        let labels: std::collections::HashSet<_> =
            blocks.iter().map(|(_, b)| b.label.clone()).collect();
        assert_eq!(labels.len(), 3, "three people on the mic: {labels:?}");
        assert!(
            voices.iter().all(|v| labels.contains(&v.label)),
            "every voice belongs to a label"
        );
    }
}
