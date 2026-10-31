//! Live captions (VRT-16, doc-2 §1.3 pass 1). Each recorded track feeds its own worker
//! thread: downmix to mono, resample to 16 kHz, cut into 4-8 s chunks, and transcribe each
//! chunk. From 4 s on, a chunk ends in the first pause (100 ms of quiet); without a
//! pause it ends by 8 s at the quietest frame, so a chunk never waits for a pause. The cut is
//! decided as the audio arrives (no look-ahead), which keeps the text close behind speech.
//! Caption times are milliseconds on the track's timeline (the same as the track file and
//! the mix).

use std::sync::mpsc;

use serde::Serialize;

use crate::audio::levels;
use crate::audio::resample::StreamResampler;
use crate::audio::{Feed, SourceKind};
use crate::engine::transcript::Word;
use crate::engine::{Engine, SAMPLE_RATE};

const RATE: usize = SAMPLE_RATE as usize;
/// A chunk is cut between 4 and 8 s after the previous cut (validated in VRT-32).
pub const MIN_CHUNK: usize = 4 * RATE;
pub const MAX_CHUNK: usize = 8 * RATE;
/// Pauses are found per 20 ms frame; the cut goes into the middle of the frame.
const FRAME: usize = RATE / 50;
/// A frame is a pause when its energy is this far below the chunk's median frame (-13 dB)...
const PAUSE_RATIO: f32 = 0.05;
/// ...or when it is this quiet in absolute terms.
const PAUSE_DB: f32 = -55.0;
/// A pause is this many quiet frames in a row (160 ms): shorter dips occur inside words
/// (stop consonants), and cutting there garbles the word on both sides (VRT-16).
const PAUSE_FRAMES: usize = 8;
/// A chunk that comes back empty although it holds sound is put back and the next chunk is
/// made this much longer: parakeet sometimes returns nothing for ~4 s pieces that start or
/// end inside an utterance (VRT-16), while the longer piece works.
const EXTEND: usize = 2 * RATE;
/// Up to this length; beyond it an empty result is accepted.
const MAX_EXTENDED: usize = 12 * RATE;
/// The rest at the end of a recording is only transcribed when it is this long.
const MIN_TAIL: usize = RATE / 2;
/// After a cut outside a pause, words ending this close to the cut are dropped and their
/// audio goes into the next chunk, so a word cut in two is transcribed whole (VRT-16).
const REDO_TAIL_MS: u32 = 300;
/// Chunks quieter than this are not sent to the engine (nothing to transcribe). Quiet
/// remote speech can sit around -50 dBFS on the system track (VRT-16), silence below -65.
const SILENCE_DB: f32 = -60.0;

/// 16 kHz mono audio starting `start` samples into the track.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub start: u64,
    pub samples: Vec<f32>,
    /// Cut in a pause (or at the end of the recording), not in running speech.
    pub at_pause: bool,
}

impl Chunk {
    pub fn start_ms(&self) -> u32 {
        (self.start * 1000 / RATE as u64) as u32
    }
}

/// Cuts a continuous 16 kHz stream into chunks.
pub struct Chunker {
    buf: Vec<f32>,
    start: u64,
    /// Energy of each complete 20 ms frame of `buf`.
    energies: Vec<f32>,
    /// Frames before this index were already checked for a pause.
    checked: usize,
    /// Length bounds of the next chunk (MIN_CHUNK..MAX_CHUNK unless extended).
    lo: usize,
    hi: usize,
}

impl Default for Chunker {
    fn default() -> Self {
        Self {
            buf: Vec::new(),
            start: 0,
            energies: Vec::new(),
            checked: 0,
            lo: MIN_CHUNK,
            hi: MAX_CHUNK,
        }
    }
}

impl Chunker {
    pub fn push(&mut self, samples: &[f32]) -> Vec<Chunk> {
        self.buf.extend_from_slice(samples);
        let mut out = Vec::new();
        loop {
            while (self.energies.len() + 1) * FRAME <= self.buf.len() {
                let i = self.energies.len() * FRAME;
                self.energies.push(frame_energy(&self.buf[i..i + FRAME]));
            }
            let cut = self.first_pause().map(|c| (c, true)).or_else(|| {
                (self.buf.len() >= self.hi + HALF_WINDOW)
                    .then(|| (quietest_cut(&self.buf, self.lo, self.hi), false))
            });
            match cut {
                Some((cut, at_pause)) => out.push(self.take(cut, at_pause)),
                None => break,
            }
        }
        out
    }

    /// The middle of the first pause that lies between the chunk's length bounds.
    fn first_pause(&mut self) -> Option<usize> {
        // Frame k completes a pause of frames k+1-PAUSE_FRAMES..=k, cut in its middle.
        let cut_at = |k: usize| (k + 1) * FRAME - PAUSE_FRAMES * FRAME / 2;
        let first = (self.lo + PAUSE_FRAMES * FRAME / 2) / FRAME - 1;
        let from = self.checked.max(first);
        self.checked = self.energies.len();
        if from >= self.energies.len() {
            return None;
        }
        let mut sorted = self.energies[..from].to_vec();
        sorted.sort_by(f32::total_cmp);
        let quiet_below = (sorted[sorted.len() / 2] * PAUSE_RATIO).max(10f32.powf(PAUSE_DB / 10.0));
        let quiet = |k: usize| self.energies[k] <= quiet_below;
        (from..self.energies.len())
            .take_while(|&k| cut_at(k) <= self.hi)
            .find(|&k| (k + 1 - PAUSE_FRAMES..=k).all(quiet))
            .map(cut_at)
    }

    /// The rest, at the end of the recording.
    pub fn finish(mut self) -> Option<Chunk> {
        let n = self.buf.len();
        (n >= MIN_TAIL).then(|| self.take(n, true))
    }

    /// Put the end of `chunk` (from sample `from` on) back in front of the buffered audio,
    /// so it is transcribed again as the start of the next chunk.
    pub fn rewind(&mut self, chunk: &Chunk, from: usize) {
        let mut buf = chunk.samples[from..].to_vec();
        buf.append(&mut self.buf);
        self.buf = buf;
        self.start = chunk.start + from as u64;
        self.energies.clear();
        self.checked = 0;
    }

    /// Put all of `chunk` back and make the next chunk at least EXTEND longer. Returns false
    /// (and does nothing) once the chunk is already MAX_EXTENDED long.
    pub fn extend(&mut self, chunk: &Chunk) -> bool {
        let lo = chunk.samples.len() + EXTEND;
        if lo > MAX_EXTENDED {
            return false;
        }
        self.rewind(chunk, 0);
        self.lo = lo;
        self.hi = self.hi.max(lo + EXTEND);
        true
    }

    fn take(&mut self, n: usize, at_pause: bool) -> Chunk {
        let chunk = Chunk {
            start: self.start,
            samples: self.buf.drain(..n).collect(),
            at_pause,
        };
        self.start += n as u64;
        self.energies.clear();
        self.checked = 0;
        self.lo = MIN_CHUNK;
        self.hi = MAX_CHUNK;
        chunk
    }
}

/// Mean square around the frame's mean (a DC offset is not sound).
fn frame_energy(frame: &[f32]) -> f32 {
    let mean = frame.iter().sum::<f32>() / frame.len() as f32;
    frame.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>() / frame.len() as f32
}

/// Half the window the fallback cut looks at: the quietest stretch of pause length.
const HALF_WINDOW: usize = PAUSE_FRAMES * FRAME / 2;

/// The fallback cut position in [lo, hi]: the middle of the quietest stretch of pause length
/// (energy around the mean, so a DC offset doesn't count as sound). A single quiet frame can
/// sit inside a word; a longer stretch is more likely between words.
fn quietest_cut(buf: &[f32], lo: usize, hi: usize) -> usize {
    let mut best = (f32::MAX, hi);
    let mut cut = lo;
    while cut <= hi && cut + HALF_WINDOW <= buf.len() {
        let energy = frame_energy(&buf[cut - HALF_WINDOW..cut + HALF_WINDOW]);
        if energy < best.0 {
            best = (energy, cut);
        }
        cut += FRAME;
    }
    best.1
}

/// Average interleaved channels into mono.
pub fn downmix(interleaved: &[f32], channels: u16, out: &mut Vec<f32>) {
    let ch = usize::from(channels.max(1));
    if ch == 1 {
        out.extend_from_slice(interleaved);
    } else {
        out.extend(
            interleaved
                .chunks_exact(ch)
                .map(|f| f.iter().sum::<f32>() / ch as f32),
        );
    }
}

/// What to transcribe again after a chunk.
enum Redo {
    /// The words from this sample offset on (a word cut in two at a forced cut).
    From(usize),
    /// The whole chunk, as part of a longer one (it came back empty).
    Extend,
}

/// Which track a worker transcribes.
#[derive(Debug, Clone)]
pub struct TrackRef {
    pub meeting_id: i64,
    pub track_id: i64,
    pub source_id: String,
    pub kind: SourceKind,
}

/// One transcribed chunk (event `transcript:caption`). Times are track milliseconds.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Caption {
    pub meeting_id: i64,
    pub track_id: i64,
    pub source_id: String,
    pub kind: SourceKind,
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,
    pub words: Vec<Word>,
}

/// Start a caption worker for one track and return the feed the recorder writes into.
/// The worker ends after the feed closes (the recording stopped) and its last chunk is done.
/// Engine errors go to `on_error`; they never affect the recording.
pub fn spawn<E: Engine + Send + 'static>(
    track: TrackRef,
    sample_rate: u32,
    channels: u16,
    engine: E,
    on_caption: impl Fn(Caption) + Send + 'static,
    on_error: impl Fn(String) + Send + 'static,
) -> std::io::Result<(Feed, std::thread::JoinHandle<()>)> {
    let (tx, rx) = mpsc::channel::<Vec<f32>>();
    let name = format!("veritas-captions-{}", track.track_id);
    let handle = std::thread::Builder::new().name(name).spawn(move || {
        let mut resampler = StreamResampler::new(sample_rate, SAMPLE_RATE);
        let mut chunker = Chunker::default();
        let (mut mono, mut pcm) = (Vec::new(), Vec::new());
        // Transcribe a chunk; returns what the chunker should redo, if anything.
        let transcribe = |chunk: &Chunk| -> Option<Redo> {
            if levels::measure(&chunk.samples).rms_db < SILENCE_DB {
                return None;
            }
            let t = match tauri::async_runtime::block_on(engine.transcribe_chunk(&chunk.samples)) {
                Ok(t) => t,
                Err(e) => {
                    on_error(e);
                    return None;
                }
            };
            let mut words = t.words;
            if words.is_empty() {
                // Sound but no words: try again as part of a longer chunk.
                return Some(Redo::Extend);
            }
            let mut rewind = None;
            if !chunk.at_pause {
                let len_ms = (chunk.samples.len() * 1000 / RATE) as u32;
                if let Some(i) = words.iter().position(|w| w.end_ms + REDO_TAIL_MS > len_ms)
                    && i > 0
                {
                    // Redo from just after the last kept word.
                    let from_ms = words[i - 1]
                        .end_ms
                        .max(words[i].start_ms.saturating_sub(100));
                    rewind = Some(Redo::From(from_ms as usize * RATE / 1000));
                    words.truncate(i);
                }
            }
            if words.is_empty() {
                return rewind;
            }
            let offset = chunk.start_ms();
            for w in &mut words {
                w.start_ms += offset;
                w.end_ms += offset;
            }
            on_caption(Caption {
                meeting_id: track.meeting_id,
                track_id: track.track_id,
                source_id: track.source_id.clone(),
                kind: track.kind,
                start_ms: words[0].start_ms,
                end_ms: words.iter().map(|w| w.end_ms).max().unwrap_or(offset),
                text: words
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
                words,
            });
            rewind
        };
        let handle_all = |chunker: &mut Chunker, chunks: Vec<Chunk>| {
            let last = chunks.len().saturating_sub(1);
            for (i, chunk) in chunks.iter().enumerate() {
                // Only the newest chunk can be redone (the buffer follows it).
                match transcribe(chunk) {
                    Some(Redo::From(from)) if i == last => chunker.rewind(chunk, from),
                    Some(Redo::Extend) if i == last => {
                        chunker.extend(chunk);
                    }
                    _ => {}
                }
            }
        };
        for block in rx {
            mono.clear();
            downmix(&block, channels, &mut mono);
            pcm.clear();
            resampler.process(&mono, &mut pcm);
            let chunks = chunker.push(&pcm);
            handle_all(&mut chunker, chunks);
        }
        // The recording stopped: flush the resampler and the last chunk.
        pcm.clear();
        resampler.finish(&mut pcm);
        let chunks = chunker.push(&pcm);
        handle_all(&mut chunker, chunks);
        if let Some(chunk) = chunker.finish() {
            transcribe(&chunk);
        }
    })?;
    Ok((tx, handle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Diarization, FileTranscript, Transcript};
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    /// Noise everywhere except silent stretches at the given sample positions.
    fn noise_with_gaps(len: usize, gaps: &[(usize, usize)]) -> Vec<f32> {
        let mut x = 1u32;
        (0..len)
            .map(|i| {
                x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                if gaps.iter().any(|&(a, b)| i >= a && i < b) {
                    0.0
                } else {
                    (x >> 8) as f32 / (1u32 << 24) as f32 - 0.5
                }
            })
            .collect()
    }

    #[test]
    fn cuts_at_the_first_pause_after_4_seconds_without_waiting() {
        let early = (3 * RATE, 3 * RATE + 12 * FRAME); // before 4 s: ignored
        let dip = (4 * RATE + 3000, 4 * RATE + 3000 + 5 * FRAME); // inside a word: too short
        let first = (4 * RATE + 8000, 4 * RATE + 8000 + 10 * FRAME);
        let second = (6 * RATE, 6 * RATE + 12 * FRAME); // longer, but later
        let audio = noise_with_gaps(9 * RATE, &[early, dip, first, second]);
        let mut chunker = Chunker::default();
        let mut pushed = 0;
        let mut chunks = Vec::new();
        for block in audio.chunks(RATE / 20) {
            pushed += block.len();
            chunks.extend(chunker.push(block));
            if !chunks.is_empty() {
                break;
            }
        }
        let cut = chunks[0].samples.len();
        assert!(
            cut >= first.0 && cut <= first.1,
            "cut at {cut}, pause {first:?}"
        );
        assert!(
            pushed <= first.1 + RATE / 20,
            "emitted {} ms after the pause",
            (pushed - first.1) * 1000 / RATE
        );
        let rest = chunker.push(&audio[pushed..]);
        assert!(rest.is_empty(), "the next chunk needs 4 s first");
        let tail = chunker.finish().unwrap();
        assert_eq!(tail.start, cut as u64, "chunks are contiguous");
        assert_eq!(cut + tail.samples.len(), audio.len(), "nothing lost");
    }

    #[test]
    fn without_a_pause_the_quietest_frame_is_used_by_8_seconds() {
        // Loud noise with a quieter (but not pause-quiet) stretch at 6.5 s.
        let mut audio = noise_with_gaps(10 * RATE, &[]);
        for x in &mut audio[6 * RATE + 8000..6 * RATE + 8000 + 2 * FRAME] {
            *x *= 0.5;
        }
        let mut chunker = Chunker::default();
        let chunks = chunker.push(&audio);
        let cut = chunks[0].samples.len();
        assert!(
            (6 * RATE + 8000..=6 * RATE + 8000 + 2 * FRAME).contains(&cut),
            "cut at {cut}"
        );
    }

    #[test]
    fn without_a_pause_chunks_stay_within_4_to_8_seconds() {
        let audio = noise_with_gaps(60 * RATE, &[]);
        let mut chunker = Chunker::default();
        let mut chunks = Vec::new();
        for block in audio.chunks(RATE / 20) {
            chunks.extend(chunker.push(block));
        }
        assert!(chunks.len() >= 7);
        let mut next = 0;
        for c in &chunks {
            assert_eq!(c.start, next);
            assert!(
                (MIN_CHUNK..=MAX_CHUNK).contains(&c.samples.len()),
                "{}",
                c.samples.len()
            );
            next += c.samples.len() as u64;
        }
    }

    #[test]
    fn a_short_tail_is_dropped_and_a_dc_offset_is_not_sound() {
        let mut chunker = Chunker::default();
        assert!(chunker.push(&vec![0.1; MIN_TAIL - 1]).is_empty());
        assert!(chunker.finish().is_none());
        // A constant offset with a short burst of noise: the cut avoids the burst.
        let mut audio = vec![0.3f32; 9 * RATE];
        let burst = noise_with_gaps(FRAME * 4, &[]);
        audio[6 * RATE..6 * RATE + burst.len()].copy_from_slice(&burst);
        let cut = quietest_cut(&audio, MIN_CHUNK, MAX_CHUNK);
        assert!(!(6 * RATE - FRAME..6 * RATE + 5 * FRAME).contains(&cut));
    }

    #[test]
    fn downmix_averages_channels() {
        let mut out = Vec::new();
        downmix(&[1.0, 0.0, 0.5, 0.5], 2, &mut out);
        assert_eq!(out, [0.5, 0.5]);
        downmix(&[0.25], 1, &mut out);
        assert_eq!(out, [0.5, 0.5, 0.25]);
    }

    /// Answers every chunk with one word 100-300 ms into it (plus, with `tail`, a word that
    /// ends 100 ms before the end); fails on request.
    #[derive(Clone)]
    struct FakeEngine {
        calls: Arc<Mutex<Vec<usize>>>,
        fail: bool,
        tail: bool,
    }

    impl Engine for FakeEngine {
        async fn transcribe_chunk(&self, samples: &[f32]) -> Result<Transcript, String> {
            self.calls.lock().unwrap().push(samples.len());
            if self.fail {
                return Err("engine stopped".into());
            }
            let mut words = vec![Word {
                text: "hallo".into(),
                start_ms: 100,
                end_ms: 300,
                conf: 1.0,
            }];
            if self.tail {
                let len_ms = (samples.len() * 1000 / RATE) as u32;
                words.push(Word {
                    text: "half".into(),
                    start_ms: len_ms - 400,
                    end_ms: len_ms - 100,
                    conf: 1.0,
                });
            }
            Ok(Transcript {
                text: "hallo".into(),
                words,
            })
        }
        async fn transcribe_file(&self, _: &Path) -> Result<FileTranscript, String> {
            unreachable!()
        }
        async fn speaker_profiles(&self, _: &Path) -> Result<Diarization, String> {
            unreachable!()
        }
    }

    fn track() -> TrackRef {
        TrackRef {
            meeting_id: 1,
            track_id: 2,
            source_id: "mic".into(),
            kind: SourceKind::Mic,
        }
    }

    #[test]
    fn the_worker_resamples_chunks_and_offsets_word_times() {
        let engine = FakeEngine {
            calls: Arc::default(),
            fail: false,
            tail: false,
        };
        let captions = Arc::new(Mutex::new(Vec::new()));
        let sink = captions.clone();
        let (feed, worker) = spawn(
            track(),
            48_000,
            2,
            engine.clone(),
            move |c| sink.lock().unwrap().push(c),
            |e| panic!("{e}"),
        )
        .unwrap();
        // 20 s of stereo 48 kHz noise, then 10 s of silence (not sent), in 50 ms blocks.
        let mono = noise_with_gaps(20 * 48_000, &[]);
        let stereo: Vec<f32> = mono.iter().flat_map(|&s| [s, s]).collect();
        for block in stereo.chunks(4800) {
            feed.send(block.to_vec()).unwrap();
        }
        for _ in 0..200 {
            feed.send(vec![0.0; 4800]).unwrap();
        }
        drop(feed);
        worker.join().unwrap();
        let calls = engine.calls.lock().unwrap().clone();
        let captions = captions.lock().unwrap().clone();
        let total: usize = calls.iter().sum();
        assert!(
            calls.iter().all(|&n| (MIN_CHUNK..=MAX_CHUNK).contains(&n)),
            "{calls:?}"
        );
        assert!(
            (20 * RATE - MAX_CHUNK..=21 * RATE + MAX_CHUNK).contains(&total),
            "{total}"
        );
        assert_eq!(captions.len(), calls.len());
        assert_eq!(captions[0].start_ms, 100);
        let starts: u32 = calls[0] as u32 * 1000 / RATE as u32;
        assert_eq!(
            captions[1].start_ms,
            starts + 100,
            "second chunk offset by the first's length"
        );
        assert_eq!(
            (captions[0].track_id, captions[0].kind),
            (2, SourceKind::Mic)
        );
    }

    #[test]
    fn a_word_at_a_forced_cut_is_dropped_and_transcribed_again_in_the_next_chunk() {
        let engine = FakeEngine {
            calls: Arc::default(),
            fail: false,
            tail: true,
        };
        let captions = Arc::new(Mutex::new(Vec::new()));
        let sink = captions.clone();
        let (feed, worker) = spawn(
            track(),
            16_000,
            1,
            engine.clone(),
            move |c| sink.lock().unwrap().push(c),
            |e| panic!("{e}"),
        )
        .unwrap();
        // Noise without pauses: every cut is forced.
        for block in noise_with_gaps(30 * RATE, &[]).chunks(800) {
            feed.send(block.to_vec()).unwrap();
        }
        drop(feed);
        worker.join().unwrap();
        let calls = engine.calls.lock().unwrap().clone();
        let captions = captions.lock().unwrap().clone();
        assert!(
            calls.iter().sum::<usize>() > 30 * RATE,
            "the redone tails are transcribed twice"
        );
        let last = captions.len() - 1;
        for (i, c) in captions.iter().enumerate() {
            let texts: Vec<_> = c.words.iter().map(|w| w.text.as_str()).collect();
            // The last chunk ends the recording, so its tail word is kept.
            let expected = if i == last {
                vec!["hallo", "half"]
            } else {
                vec!["hallo"]
            };
            assert_eq!(texts, expected, "caption {i}");
        }
        // The second chunk starts 100 ms before the dropped word (500 ms before the first's end).
        let first_len_ms = (calls[0] * 1000 / RATE) as u32;
        assert_eq!(captions[1].start_ms, first_len_ms - 500 + 100);
    }

    #[test]
    fn an_empty_chunk_is_retried_as_part_of_a_longer_one() {
        // Pauses every 4.5 s would give ~4.5 s chunks; the fake engine only answers >= 6 s.
        let gaps: Vec<_> = (1..8)
            .map(|i| (i * 9 * RATE / 2, i * 9 * RATE / 2 + 12 * FRAME))
            .collect();
        let audio = noise_with_gaps(36 * RATE, &gaps);
        let lengths = Arc::new(Mutex::new(Vec::new()));
        let seen = lengths.clone();
        let mut chunker = Chunker::default();
        let mut captions = Vec::new();
        for block in audio.chunks(800) {
            for chunk in chunker.push(block) {
                seen.lock().unwrap().push(chunk.samples.len());
                if chunk.samples.len() < 6 * RATE {
                    assert!(chunker.extend(&chunk));
                } else {
                    captions.push(chunk);
                }
            }
        }
        let lengths = lengths.lock().unwrap().clone();
        assert!(!captions.is_empty(), "{lengths:?}");
        assert!(captions.iter().all(|c| c.samples.len() >= 6 * RATE));
        let mut next = 0;
        for c in &captions {
            assert_eq!(
                c.start, next,
                "an extended chunk starts where the empty one did"
            );
            next += c.samples.len() as u64;
        }
        // At 12 s an empty chunk is accepted.
        let long = Chunk {
            start: 0,
            samples: vec![0.0; MAX_EXTENDED],
            at_pause: true,
        };
        assert!(!Chunker::default().extend(&long));
    }

    #[test]
    fn rewind_puts_the_tail_back_in_front() {
        let mut chunker = Chunker::default();
        let audio = noise_with_gaps(9 * RATE, &[]);
        let chunk = chunker.push(&audio).remove(0);
        assert!(!chunk.at_pause);
        let len = chunk.samples.len();
        chunker.rewind(&chunk, len - RATE);
        let tail = chunker.finish().unwrap();
        assert_eq!(tail.start, (len - RATE) as u64);
        assert_eq!(tail.samples, audio[len - RATE..]);
    }

    #[test]
    fn engine_errors_are_reported_and_the_worker_keeps_going() {
        let engine = FakeEngine {
            calls: Arc::default(),
            fail: true,
            tail: false,
        };
        let errors = Arc::new(Mutex::new(Vec::new()));
        let sink = errors.clone();
        let (feed, worker) = spawn(
            track(),
            16_000,
            1,
            engine.clone(),
            |_| panic!("no captions"),
            move |e| sink.lock().unwrap().push(e),
        )
        .unwrap();
        for block in noise_with_gaps(17 * RATE, &[]).chunks(800) {
            feed.send(block.to_vec()).unwrap();
        }
        drop(feed);
        worker.join().unwrap();
        assert!(errors.lock().unwrap().len() >= 2, "every chunk was tried");
    }
}
