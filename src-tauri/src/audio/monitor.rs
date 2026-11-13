//! Level monitor for Settings > Sources: captures the selected sources and reports their
//! levels (~20 Hz) without writing anything. Stops when dropped.

use std::sync::mpsc;
use std::thread::JoinHandle;

use super::capture::{AudioSource, CpalSource};
use super::levels::{self, InputWatch};
use super::recorder::{LevelEvent, RecorderError, TICK};
use super::ring::RingReader;
use super::sources::SourceInfo;

pub struct Monitor {
    stop: mpsc::Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl Monitor {
    /// Start monitoring; returns once every source is capturing.
    pub fn start(
        sources: Vec<SourceInfo>,
        mut on_level: impl FnMut(LevelEvent) + Send + 'static,
    ) -> Result<Self, RecorderError> {
        if sources.is_empty() {
            return Err(RecorderError::NoSources);
        }
        let (stop, rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), RecorderError>>();
        let thread = std::thread::Builder::new()
            .name("manent-monitor".into())
            .spawn(move || {
                let mut streams = Vec::new();
                let mut inputs: Vec<(SourceInfo, RingReader, InputWatch)> = Vec::new();
                let opened = (|| -> Result<(), RecorderError> {
                    for info in sources {
                        let (mut source, reader) = CpalSource::open(info.clone())?;
                        source.start()?;
                        streams.push(source);
                        inputs.push((info, reader, InputWatch::default()));
                    }
                    Ok(())
                })();
                let ok = opened.is_ok();
                let _ = ready_tx.send(opened);
                if !ok {
                    return;
                }
                let mut buf = Vec::new();
                while let Err(mpsc::RecvTimeoutError::Timeout) = rx.recv_timeout(TICK) {
                    for (info, reader, watch) in &mut inputs {
                        on_level(level_of(info, reader, watch, &mut buf));
                    }
                }
                for s in &mut streams {
                    let _ = s.stop();
                }
            })?;
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                stop,
                thread: Some(thread),
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => Err(RecorderError::ThreadGone),
        }
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Drain what arrived since the last tick and measure it.
fn level_of(
    info: &SourceInfo,
    reader: &mut RingReader,
    watch: &mut InputWatch,
    buf: &mut Vec<f32>,
) -> LevelEvent {
    buf.clear();
    reader.drain_into(buf);
    let level = levels::measure(buf);
    LevelEvent {
        source_id: info.id.clone(),
        kind: info.kind,
        rms_db: level.rms_db,
        peak_db: level.peak_db,
        paused: false,
        muted: false,
        no_input: watch.tick(buf.len(), TICK),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::ring::sample_ring;
    use crate::audio::sources::SourceKind;

    #[test]
    fn level_reflects_only_new_samples() {
        let info = SourceInfo {
            id: "mic".into(),
            name: "Mic".into(),
            kind: SourceKind::Mic,
            is_default: true,
        };
        let (mut w, mut r) = sample_ring(1024);
        let mut buf = Vec::new();
        let mut watch = InputWatch::default();
        let square: Vec<f32> = (0..100)
            .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
            .collect();
        w.push(&square);
        let first = level_of(&info, &mut r, &mut watch, &mut buf);
        assert!((first.rms_db - -6.02).abs() < 0.05);
        let silent = level_of(&info, &mut r, &mut watch, &mut buf);
        assert_eq!(silent.rms_db, levels::FLOOR_DB, "nothing new -> floor");
        assert_eq!(silent.source_id, "mic");
        assert!(!silent.no_input, "it delivered before");
    }

    /// Needs audio hardware: `cargo test -- --ignored audio_hardware_monitor --nocapture`.
    #[test]
    #[ignore]
    fn audio_hardware_monitor_reports_levels() {
        use std::sync::{Arc, Mutex};
        let sources = crate::audio::list_sources().unwrap();
        let mic = sources
            .iter()
            .find(|s| s.kind == SourceKind::Mic && s.is_default)
            .cloned()
            .unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let monitor = Monitor::start(vec![mic], move |e| sink.lock().unwrap().push(e)).unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        drop(monitor);
        let n = events.lock().unwrap().len();
        let count_after_drop = n;
        std::thread::sleep(std::time::Duration::from_millis(300));
        println!("{n} level events in 1 s");
        assert!((15..=25).contains(&n));
        assert_eq!(
            events.lock().unwrap().len(),
            count_after_drop,
            "stops on drop"
        );
    }
}
