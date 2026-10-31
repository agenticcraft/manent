//! Recording a meeting: one FLAC track per source plus live level events (doc-2 §1.2).
//!
//! A dedicated capture thread owns the cpal streams (they are not `Send` on every platform).
//! Every tick (~20 Hz) it drains each source's ring into that source's FLAC file and reports
//! the level. Pausing keeps the streams running but discards samples on all tracks at once,
//! so the files stay valid and the tracks stay aligned.
//!
//! A track can also feed live transcription (VRT-16): it sends exactly what it writes
//! (silence while muted, nothing while paused) to a consumer, so caption times line up with
//! the track file.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use serde::Serialize;

use super::capture::{AudioError, AudioSource, CpalSource};
use super::flac::{FlacError, FlacWriter};
use super::levels;
use super::ring::RingReader;
use super::sources::{SourceInfo, SourceKind};

/// Level events are sent at this interval (~20 Hz).
pub const TICK: Duration = Duration::from_millis(50);

#[derive(Debug, thiserror::Error)]
pub enum RecorderError {
    #[error(transparent)]
    Audio(#[from] AudioError),
    #[error(transparent)]
    Flac(#[from] FlacError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("no sources selected")]
    NoSources,
    #[error("capture thread stopped unexpectedly")]
    ThreadGone,
}

/// Live level of one source, sent to the UI as the `audio:level` event.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LevelEvent {
    pub source_id: String,
    pub kind: SourceKind,
    pub rms_db: f32,
    pub peak_db: f32,
    pub paused: bool,
    /// A muted source records silence (tracks stay aligned); its level reflects the input.
    pub muted: bool,
}

/// What was written for one source.
#[derive(Debug, Clone, Serialize)]
pub struct TrackSummary {
    pub source_id: String,
    pub kind: SourceKind,
    pub file: PathBuf,
    pub seconds: f64,
    pub dropped_samples: u64,
}

/// File name per source: `mic.flac`, `system.flac`, then `mic-2.flac`, ... for extra sources.
pub fn track_file_names(sources: &[SourceInfo]) -> Vec<String> {
    let mut seen = std::collections::HashMap::new();
    sources
        .iter()
        .map(|s| {
            let base = match s.kind {
                SourceKind::Mic => "mic",
                SourceKind::Loopback => "system",
            };
            let n = seen.entry(base).and_modify(|n| *n += 1).or_insert(1);
            if *n == 1 {
                format!("{base}.flac")
            } else {
                format!("{base}-{n}.flac")
            }
        })
        .collect()
}

/// Interleaved blocks of what a track writes, at the source's rate and channel count.
pub type Feed = mpsc::Sender<Vec<f32>>;

/// The consumer side of one track: ring -> FLAC file (and optionally a feed).
pub struct Track {
    pub info: SourceInfo,
    pub file: PathBuf,
    reader: RingReader,
    writer: FlacWriter,
    buf: Vec<f32>,
    muted: bool,
    silence: Vec<f32>,
    feed: Option<Feed>,
}

impl Track {
    pub fn new(
        info: SourceInfo,
        file: PathBuf,
        reader: RingReader,
        sample_rate: u32,
        channels: u16,
    ) -> Result<Self, RecorderError> {
        let writer = FlacWriter::create(&file, sample_rate, channels)?;
        Ok(Self {
            info,
            file,
            reader,
            writer,
            buf: Vec::with_capacity(sample_rate as usize * channels as usize / 10),
            muted: false,
            silence: Vec::new(),
            feed: None,
        })
    }

    /// Also send everything written to `feed` (dropped when the track finishes).
    pub fn set_feed(&mut self, feed: Option<Feed>) {
        self.feed = feed;
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    /// Drain the ring; write unless paused (silence instead of audio while muted); return the
    /// level of what arrived.
    pub fn tick(&mut self, paused: bool) -> Result<LevelEvent, RecorderError> {
        self.buf.clear();
        self.reader.drain_into(&mut self.buf);
        if !paused {
            let written = if self.muted {
                self.silence.clear();
                self.silence.resize(self.buf.len(), 0.0);
                &self.silence
            } else {
                &self.buf
            };
            self.writer.write(written)?;
            if let Some(feed) = &self.feed
                && !written.is_empty()
                && feed.send(written.clone()).is_err()
            {
                // The consumer is gone (e.g. transcription failed): keep recording.
                self.feed = None;
            }
        }
        let level = levels::measure(&self.buf);
        Ok(LevelEvent {
            source_id: self.info.id.clone(),
            kind: self.info.kind,
            rms_db: level.rms_db,
            peak_db: level.peak_db,
            paused,
            muted: self.muted,
        })
    }

    pub fn finish(self) -> Result<TrackSummary, RecorderError> {
        let dropped = self.reader.dropped();
        let seconds = self.writer.seconds();
        self.writer.finish()?;
        Ok(TrackSummary {
            source_id: self.info.id,
            kind: self.info.kind,
            file: self.file,
            seconds,
            dropped_samples: dropped,
        })
    }
}

enum Control {
    Pause,
    Resume,
    SetMuted(String, bool),
    Stop,
}

/// A running recording. Dropping it without `stop` stops the thread but leaves the files'
/// headers without totals; always call `stop`.
pub struct Recording {
    control: mpsc::Sender<Control>,
    thread: Option<JoinHandle<Result<Vec<TrackSummary>, RecorderError>>>,
}

impl Recording {
    /// Start recording `sources` into `dir`. Returns once every source is capturing.
    /// `on_track(index, source, sample_rate, channels)` runs once per opened source and may
    /// return a feed for that track (live transcription).
    pub fn start(
        dir: &Path,
        sources: Vec<SourceInfo>,
        mut on_level: impl FnMut(LevelEvent) + Send + 'static,
        mut on_track: impl FnMut(usize, &SourceInfo, u32, u16) -> Option<Feed> + Send + 'static,
    ) -> Result<Self, RecorderError> {
        if sources.is_empty() {
            return Err(RecorderError::NoSources);
        }
        std::fs::create_dir_all(dir)?;
        let files: Vec<PathBuf> = track_file_names(&sources)
            .into_iter()
            .map(|n| dir.join(n))
            .collect();
        let (control, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), RecorderError>>();

        let thread = std::thread::Builder::new()
            .name("veritas-capture".into())
            .spawn(move || {
                let mut streams = Vec::new();
                let mut tracks = Vec::new();
                let opened = (|| -> Result<(), RecorderError> {
                    for (i, (info, file)) in sources.into_iter().zip(files).enumerate() {
                        let (mut source, reader) = CpalSource::open(info.clone())?;
                        let f = source.format();
                        let feed = on_track(i, &info, f.sample_rate, f.channels);
                        let mut track = Track::new(info, file, reader, f.sample_rate, f.channels)?;
                        track.set_feed(feed);
                        tracks.push(track);
                        source.start()?;
                        streams.push(source);
                    }
                    Ok(())
                })();
                if let Err(e) = opened {
                    let _ = ready_tx.send(Err(e));
                    return Ok(Vec::new());
                }
                let _ = ready_tx.send(Ok(()));
                run_loop(&mut tracks, &rx, &mut on_level)?;
                for s in &mut streams {
                    s.stop()?;
                }
                // Final drain after the streams stopped, then close the files.
                for t in &mut tracks {
                    t.tick(false)?;
                }
                tracks.into_iter().map(Track::finish).collect()
            })?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                control,
                thread: Some(thread),
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => Err(RecorderError::ThreadGone),
        }
    }

    pub fn pause(&self) {
        let _ = self.control.send(Control::Pause);
    }

    pub fn resume(&self) {
        let _ = self.control.send(Control::Resume);
    }

    /// Mute or unmute one source by id; a muted source records silence.
    pub fn set_muted(&self, source_id: &str, muted: bool) {
        let _ = self
            .control
            .send(Control::SetMuted(source_id.to_string(), muted));
    }

    /// Stop capturing, finish the files and return what was written.
    pub fn stop(mut self) -> Result<Vec<TrackSummary>, RecorderError> {
        let _ = self.control.send(Control::Stop);
        let thread = self.thread.take().ok_or(RecorderError::ThreadGone)?;
        thread.join().map_err(|_| RecorderError::ThreadGone)?
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = self.control.send(Control::Stop);
            let _ = thread.join();
        }
    }
}

/// Tick every track until `Stop`; `Pause`/`Resume` toggle writing.
fn run_loop(
    tracks: &mut [Track],
    control: &mpsc::Receiver<Control>,
    on_level: &mut impl FnMut(LevelEvent),
) -> Result<(), RecorderError> {
    let mut paused = false;
    loop {
        match control.recv_timeout(TICK) {
            Ok(Control::Pause) => paused = true,
            Ok(Control::Resume) => paused = false,
            Ok(Control::SetMuted(id, muted)) => {
                for t in tracks.iter_mut().filter(|t| t.info.id == id) {
                    t.set_muted(muted);
                }
            }
            Ok(Control::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        for t in tracks.iter_mut() {
            on_level(t.tick(paused)?);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::ring::sample_ring;
    use crate::test_util::temp_dir as tempfile_dir;

    fn info(id: &str, kind: SourceKind) -> SourceInfo {
        SourceInfo {
            id: id.into(),
            name: id.into(),
            kind,
            is_default: false,
        }
    }

    #[test]
    fn file_names_per_kind() {
        let s = [
            info("a", SourceKind::Mic),
            info("b", SourceKind::Loopback),
            info("c", SourceKind::Mic),
        ];
        assert_eq!(
            track_file_names(&s),
            ["mic.flac", "system.flac", "mic-2.flac"]
        );
    }

    #[test]
    fn paused_audio_is_not_written_and_files_stay_valid() {
        let dir = tempfile_dir();
        let (mut w, r) = sample_ring(16_000);
        let mut track = Track::new(
            info("mic", SourceKind::Mic),
            dir.join("mic.flac"),
            r,
            16_000,
            1,
        )
        .unwrap();
        let block: Vec<f32> = (0..1600)
            .map(|i| if i % 2 == 0 { 0.25 } else { -0.25 })
            .collect(); // 100 ms
        for paused in [false, false, true, true, true, false] {
            w.push(&block);
            let ev = track.tick(paused).unwrap();
            assert_eq!(ev.paused, paused);
            assert!(
                (ev.rms_db - -12.04).abs() < 0.1,
                "level reported while paused too"
            );
        }
        let summary = track.finish().unwrap();
        let reader = claxon::FlacReader::open(&summary.file).unwrap();
        assert_eq!(reader.streaminfo().samples, Some(3 * 1600));
        assert_eq!(summary.dropped_samples, 0);
    }

    #[test]
    fn muted_audio_is_recorded_as_silence_of_the_same_length() {
        let dir = tempfile_dir();
        let (mut w, r) = sample_ring(16_000);
        let mut track = Track::new(
            info("mic", SourceKind::Mic),
            dir.join("mic.flac"),
            r,
            16_000,
            1,
        )
        .unwrap();
        let block: Vec<f32> = (0..1600)
            .map(|i| if i % 2 == 0 { 0.25 } else { -0.25 })
            .collect();
        for muted in [false, true, true, false] {
            track.set_muted(muted);
            w.push(&block);
            let ev = track.tick(false).unwrap();
            assert_eq!(ev.muted, muted);
            assert!(
                (ev.rms_db - -12.04).abs() < 0.1,
                "level still shows the input"
            );
        }
        let summary = track.finish().unwrap();
        let mut reader = claxon::FlacReader::open(&summary.file).unwrap();
        let samples: Vec<i32> = reader.samples().map(|s| s.unwrap()).collect();
        assert_eq!(
            samples.len(),
            4 * 1600,
            "muted time is kept, so tracks stay aligned"
        );
        assert!(
            samples[1600..3200 * 2 - 1600].iter().all(|&s| s == 0),
            "muted stretch is silent"
        );
        assert!(samples[..1600].iter().all(|&s| s != 0) && samples[4800..].iter().all(|&s| s != 0));
    }

    #[test]
    fn the_feed_gets_what_is_written() {
        let dir = tempfile_dir();
        let (mut w, r) = sample_ring(16_000);
        let mut track = Track::new(
            info("mic", SourceKind::Mic),
            dir.join("mic.flac"),
            r,
            16_000,
            1,
        )
        .unwrap();
        let (tx, rx) = mpsc::channel();
        track.set_feed(Some(tx));
        let block = vec![0.5f32; 800];
        // written, paused (not written), muted (silence written)
        for (paused, muted) in [(false, false), (true, false), (false, true)] {
            track.set_muted(muted);
            w.push(&block);
            track.tick(paused).unwrap();
        }
        let got: Vec<Vec<f32>> = rx.try_iter().collect();
        assert_eq!(got.len(), 2, "nothing is fed while paused");
        assert_eq!(got[0], block);
        assert_eq!(got[1], vec![0.0; 800], "muted audio is fed as silence");
        track.finish().unwrap();
        assert!(
            rx.recv().is_err(),
            "the feed closes when the track finishes"
        );
    }

    #[test]
    fn run_loop_emits_levels_per_track_and_honours_pause() {
        let dir = tempfile_dir();
        let (mut w1, r1) = sample_ring(48_000);
        let (mut w2, r2) = sample_ring(48_000);
        let mut tracks = vec![
            Track::new(
                info("m", SourceKind::Mic),
                dir.join("mic.flac"),
                r1,
                16_000,
                1,
            )
            .unwrap(),
            Track::new(
                info("s", SourceKind::Loopback),
                dir.join("system.flac"),
                r2,
                16_000,
                1,
            )
            .unwrap(),
        ];
        let (tx, rx) = mpsc::channel();
        let feeder = std::thread::spawn(move || {
            let block = [0.1f32; 800]; // 50 ms at 16 kHz
            for i in 0..20 {
                w1.push(&block);
                w2.push(&block);
                if i == 8 {
                    tx.send(Control::Pause).unwrap();
                }
                if i == 14 {
                    tx.send(Control::Resume).unwrap();
                }
                std::thread::sleep(TICK);
            }
            tx.send(Control::Stop).unwrap();
        });
        let mut events = Vec::new();
        run_loop(&mut tracks, &rx, &mut |e| events.push(e)).unwrap();
        feeder.join().unwrap();
        for t in &mut tracks {
            t.tick(false).unwrap();
        }
        let summaries: Vec<_> = tracks.into_iter().map(|t| t.finish().unwrap()).collect();
        assert!(events.iter().any(|e| e.paused), "pause reached the loop");
        assert!(events.iter().filter(|e| e.source_id == "m").count() >= 15);
        // Both tracks dropped the same paused stretch, so they stay aligned.
        assert!((summaries[0].seconds - summaries[1].seconds).abs() < 0.06);
        assert!(summaries[0].seconds < 1.0 && summaries[0].seconds > 0.5);
    }

    /// Needs audio hardware. Records the default mic + system audio, pausing 2 s halfway.
    /// `VERITAS_RECORD_SECONDS` sets the length (default 10). Run:
    /// `cargo test -- --ignored audio_hardware_record --nocapture`
    #[test]
    #[ignore]
    fn audio_hardware_record_mic_and_system() {
        use std::sync::{Arc, Mutex};
        use std::time::Instant;
        let seconds: f64 = std::env::var("VERITAS_RECORD_SECONDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10.0);
        let sources = crate::audio::list_sources().unwrap();
        let pick = |kind| {
            sources
                .iter()
                .find(|s| s.kind == kind && s.is_default)
                .cloned()
                .unwrap()
        };
        let dir = tempfile_dir();
        let events = Arc::new(Mutex::new(Vec::<(Instant, LevelEvent)>::new()));
        let sink = events.clone();
        let rec = Recording::start(
            &dir,
            vec![pick(SourceKind::Mic), pick(SourceKind::Loopback)],
            move |e| sink.lock().unwrap().push((Instant::now(), e)),
            |_, _, _, _| None,
        )
        .unwrap();
        let t0 = Instant::now();
        let half = Duration::from_secs_f64(seconds / 2.0);
        std::thread::sleep(half);
        rec.pause();
        std::thread::sleep(Duration::from_secs(2));
        rec.resume();
        std::thread::sleep(half);
        let summaries = rec.stop().unwrap();
        let wall = t0.elapsed().as_secs_f64();
        let events = events.lock().unwrap();
        for s in &summaries {
            let reader = claxon::FlacReader::open(&s.file).expect("valid flac");
            let info = reader.streaminfo();
            let decoded = info.samples.unwrap() as f64 / info.sample_rate as f64;
            let n = events
                .iter()
                .filter(|(_, e)| e.source_id == s.source_id)
                .count();
            println!(
                "{:?}: {:.3}s written ({:.3}s decoded, {} Hz x{}), dropped {}, {} level events ({:.1}/s), {} bytes",
                s.kind,
                s.seconds,
                decoded,
                info.sample_rate,
                info.channels,
                s.dropped_samples,
                n,
                n as f64 / wall,
                std::fs::metadata(&s.file).unwrap().len()
            );
            assert_eq!(s.dropped_samples, 0);
            assert!((decoded - s.seconds).abs() < 0.1);
            assert!(
                (s.seconds - (wall - 2.0)).abs() < 0.5,
                "{} vs {}",
                s.seconds,
                wall - 2.0
            );
            assert!((n as f64 / wall - 20.0).abs() < 3.0, "level rate");
        }
        let paused = events.iter().filter(|(_, e)| e.paused).count();
        println!(
            "wall {wall:.3}s, paused level events {paused}, track difference {:.1} ms",
            (summaries[0].seconds - summaries[1].seconds).abs() * 1000.0
        );
        assert!(paused > 30, "pause reached the loop");
        assert!((summaries[0].seconds - summaries[1].seconds).abs() < 0.1);
    }
}
