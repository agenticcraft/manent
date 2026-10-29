//! Capturing one source with cpal into a lock-free ring.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use super::platform;
use super::ring::{RingReader, RingWriter, sample_ring};
use super::sources::{DeviceFacts, SourceInfo, SourceKind, classify};

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("audio source not found: {0}")]
    NotFound(String),
    #[error("audio device error: {0}")]
    Device(#[from] cpal::Error),
}

/// Sample format delivered by a source: interleaved f32 at the device's native rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamFormat {
    pub sample_rate: u32,
    pub channels: u16,
}

/// One capturable source (doc-2 §1.2). Samples go into the ring returned by `open`.
pub trait AudioSource {
    fn format(&self) -> StreamFormat;
    fn start(&mut self) -> Result<(), AudioError>;
    fn stop(&mut self) -> Result<(), AudioError>;
}

/// List the mic and loopback sources of this machine.
pub fn list_sources() -> Result<Vec<SourceInfo>, AudioError> {
    let host = cpal::default_host();
    let id_of = |d: &cpal::Device| d.id().ok().map(|id| id.to_string());
    let default_in = host.default_input_device().as_ref().and_then(id_of);
    let default_out = host.default_output_device().as_ref().and_then(id_of);
    let facts: Vec<DeviceFacts> = host
        .devices()?
        .filter_map(|d| {
            let id = id_of(&d)?;
            let name = d
                .description()
                .map(|desc| desc.name().to_string())
                .unwrap_or_else(|_| id.clone());
            Some(DeviceFacts {
                is_virtual: platform::is_virtual_device(&name),
                supports_input: d.supports_input(),
                supports_output: d.supports_output(),
                id,
                name,
            })
        })
        .collect();
    Ok(classify(
        &facts,
        default_in.as_deref(),
        default_out.as_deref(),
    ))
}

/// A cpal input stream for one source. Loopback sources open an input stream on an output
/// device, which cpal maps to the sink monitor (PipeWire) or WASAPI loopback.
pub struct CpalSource {
    info: SourceInfo,
    device: cpal::Device,
    format: StreamFormat,
    writer: Option<RingWriter>,
    stream: Option<cpal::Stream>,
    /// Set before an intentional stop: PipeWire reports "Device disconnected" while a stream
    /// is torn down, which is not an error.
    stopping: Arc<AtomicBool>,
}

/// Ring capacity in seconds of audio: enough headroom for a slow consumer.
const RING_SECONDS: usize = 2;

impl CpalSource {
    /// Open a source by id; returns the source and the reader side of its ring.
    pub fn open(info: SourceInfo) -> Result<(Self, RingReader), AudioError> {
        let host = cpal::default_host();
        let id: cpal::DeviceId = info
            .id
            .parse()
            .map_err(|_| AudioError::NotFound(info.id.clone()))?;
        let device = host
            .device_by_id(&id)
            .ok_or_else(|| AudioError::NotFound(info.id.clone()))?;
        let config = match info.kind {
            SourceKind::Mic => device.default_input_config()?,
            SourceKind::Loopback => device.default_output_config()?,
        };
        let format = StreamFormat {
            sample_rate: config.sample_rate(),
            channels: config.channels(),
        };
        let capacity = format.sample_rate as usize * format.channels as usize * RING_SECONDS;
        let (writer, reader) = sample_ring(capacity);
        let source = Self {
            info,
            device,
            format,
            writer: Some(writer),
            stream: None,
            stopping: Arc::new(AtomicBool::new(false)),
        };
        Ok((source, reader))
    }
}

impl AudioSource for CpalSource {
    fn format(&self) -> StreamFormat {
        self.format
    }

    fn start(&mut self) -> Result<(), AudioError> {
        if self.stream.is_some() {
            return Ok(());
        }
        // The writer moves into the callback; a source can be started once.
        let Some(mut writer) = self.writer.take() else {
            return Ok(());
        };
        let config = cpal::StreamConfig {
            channels: self.format.channels,
            sample_rate: self.format.sample_rate,
            buffer_size: cpal::BufferSize::Default,
        };
        let name = self.info.name.clone();
        let stopping = self.stopping.clone();
        let stream = self.device.build_input_stream(
            config,
            // Realtime thread: only push into the ring (no locks, no allocation, no blocking).
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                writer.push(data);
            },
            move |e| {
                if !stopping.load(Ordering::Relaxed) {
                    eprintln!("audio stream error on {name}: {e}");
                }
            },
            Some(std::time::Duration::from_secs(5)),
        )?;
        stream.play()?;
        self.stream = Some(stream);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        // Dropping the stream stops the callback and releases the device.
        self.stopping.store(true, Ordering::Relaxed);
        self.stream.take();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Needs real audio hardware: `cargo test -- --ignored audio_hardware`.
    #[test]
    #[ignore]
    fn audio_hardware_mic_and_loopback_capture_concurrently() {
        let sources = list_sources().expect("list sources");
        for s in &sources {
            println!("source: {s:?}");
        }
        // VERITAS_AUDIO_TEST_REAL=1 picks real devices instead of the system-default aliases.
        let want_default = std::env::var_os("VERITAS_AUDIO_TEST_REAL").is_none();
        let pick = |kind| {
            sources
                .iter()
                .find(|s| s.kind == kind && s.is_default == want_default)
                .or_else(|| sources.iter().find(|s| s.kind == kind))
                .cloned()
                .unwrap_or_else(|| panic!("no {kind:?} source in {sources:?}"))
        };
        let (mut mic, mut mic_rx) = CpalSource::open(pick(SourceKind::Mic)).unwrap();
        let (mut out, mut out_rx) = CpalSource::open(pick(SourceKind::Loopback)).unwrap();
        mic.start().unwrap();
        out.start().unwrap();
        let mut mic_buf = Vec::new();
        let mut out_buf = Vec::new();
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(100));
            mic_rx.drain_into(&mut mic_buf);
            out_rx.drain_into(&mut out_buf);
        }
        eprintln!("-- stopping sources");
        mic.stop().unwrap();
        out.stop().unwrap();
        eprintln!("-- stopped");
        let secs =
            |n: usize, f: StreamFormat| n as f64 / (f.sample_rate as f64 * f.channels as f64);
        let (mic_s, out_s) = (
            secs(mic_buf.len(), mic.format()),
            secs(out_buf.len(), out.format()),
        );
        println!(
            "mic {:?}: {mic_s:.2}s, loopback {:?}: {out_s:.2}s, dropped {}/{}",
            mic.format(),
            out.format(),
            mic_rx.dropped(),
            out_rx.dropped()
        );
        assert!(mic_s > 1.5, "mic delivered {mic_s:.2}s in 2s");
        assert!(out_s > 1.5, "loopback delivered {out_s:.2}s in 2s");
        assert_eq!(mic_rx.dropped() + out_rx.dropped(), 0);
        // After stop no more samples arrive.
        std::thread::sleep(Duration::from_millis(300));
        mic_rx.drain_into(&mut mic_buf);
        let after = mic_buf.len();
        std::thread::sleep(Duration::from_millis(300));
        mic_rx.drain_into(&mut mic_buf);
        assert_eq!(mic_buf.len(), after, "samples still arriving after stop");
    }
}
