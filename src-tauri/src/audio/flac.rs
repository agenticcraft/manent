//! Streaming FLAC writer: frames are encoded and written while recording, so a long meeting
//! never sits in memory. The 42-byte stream header is written first as a placeholder and
//! rewritten on `finish` with the total sample count, frame sizes and MD5.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

use flacenc::bitsink::ByteSink;
use flacenc::component::{BitRepr, Stream};
use flacenc::config;
use flacenc::error::Verify;
use flacenc::source::{Context, Fill, FrameBuf};

const BITS_PER_SAMPLE: usize = 16;
const BLOCK_SIZE: usize = 4096;

#[derive(Debug, thiserror::Error)]
pub enum FlacError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("flac: {0}")]
    Encode(String),
}

fn enc<E: std::fmt::Debug>(e: E) -> FlacError {
    FlacError::Encode(format!("{e:?}"))
}

pub struct FlacWriter {
    out: BufWriter<File>,
    channels: usize,
    config: flacenc::error::Verified<config::Encoder>,
    /// Holds the stream info (header) only; frames are written straight to `out`.
    header: Stream,
    framebuf: FrameBuf,
    context: Context,
    pending: Vec<i32>,
    min_frame: usize,
    max_frame: usize,
}

impl FlacWriter {
    pub fn create(path: &Path, sample_rate: u32, channels: u16) -> Result<Self, FlacError> {
        let channels = channels as usize;
        let header = Stream::new(sample_rate as usize, channels, BITS_PER_SAMPLE).map_err(enc)?;
        let mut writer = Self {
            out: BufWriter::new(File::create(path)?),
            channels,
            config: config::Encoder::default().into_verified().map_err(enc)?,
            header,
            framebuf: FrameBuf::with_size(channels, BLOCK_SIZE).map_err(enc)?,
            context: Context::new(BITS_PER_SAMPLE, channels),
            pending: Vec::with_capacity(BLOCK_SIZE * channels * 2),
            min_frame: usize::MAX,
            max_frame: 0,
        };
        writer.write_header()?;
        Ok(writer)
    }

    /// Append interleaved f32 samples in [-1, 1].
    pub fn write(&mut self, samples: &[f32]) -> Result<(), FlacError> {
        self.pending.extend(
            samples
                .iter()
                .map(|s| (s.clamp(-1.0, 1.0) * 32767.0).round() as i32),
        );
        let block = BLOCK_SIZE * self.channels;
        let mut start = 0;
        while self.pending.len() - start >= block {
            self.encode_block(start, start + block)?;
            start += block;
        }
        self.pending.drain(..start);
        Ok(())
    }

    /// Seconds of audio written so far (excluding samples still pending a full block).
    pub fn seconds(&self) -> f64 {
        self.context.total_samples() as f64 / self.header.stream_info().sample_rate() as f64
    }

    /// Flush the last partial block and rewrite the header. Returns the total frames written.
    pub fn finish(mut self) -> Result<u64, FlacError> {
        if !self.pending.is_empty() {
            let end = self.pending.len() - self.pending.len() % self.channels;
            if end > 0 {
                self.encode_block(0, end)?;
            }
        }
        let total = self.context.total_samples();
        let info = self.header.stream_info_mut();
        info.set_total_samples(total);
        info.set_md5_digest(&self.context.md5_digest());
        info.set_block_sizes(BLOCK_SIZE, BLOCK_SIZE).map_err(enc)?;
        if self.max_frame > 0 {
            info.set_frame_sizes(self.min_frame, self.max_frame)
                .map_err(enc)?;
        }
        self.out.flush()?;
        self.out.seek(SeekFrom::Start(0))?;
        self.write_header()?;
        self.out.flush()?;
        Ok(total as u64)
    }

    fn encode_block(&mut self, start: usize, end: usize) -> Result<(), FlacError> {
        let mut fill = (&mut self.framebuf, &mut self.context);
        fill.fill_interleaved(&self.pending[start..end])
            .map_err(enc)?;
        let frame_number = self.context.current_frame_number().unwrap_or(0);
        let frame = flacenc::encode_fixed_size_frame(
            &self.config,
            &self.framebuf,
            frame_number,
            self.header.stream_info(),
        )
        .map_err(enc)?;
        let mut sink = ByteSink::new();
        frame.write(&mut sink).map_err(enc)?;
        let bytes = sink.as_slice();
        self.min_frame = self.min_frame.min(bytes.len());
        self.max_frame = self.max_frame.max(bytes.len());
        self.out.write_all(bytes)?;
        Ok(())
    }

    fn write_header(&mut self) -> Result<(), FlacError> {
        let mut sink = ByteSink::new();
        self.header.write(&mut sink).map_err(enc)?;
        self.out.write_all(sink.as_slice())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::temp_dir as tempfile_dir;

    fn sine(n_frames: usize, channels: usize, rate: f32) -> Vec<f32> {
        (0..n_frames)
            .flat_map(|i| {
                let v = (i as f32 * 440.0 * std::f32::consts::TAU / rate).sin() * 0.5;
                std::iter::repeat_n(v, channels)
            })
            .collect()
    }

    fn decode(path: &Path) -> (claxon::metadata::StreamInfo, Vec<i32>) {
        let mut reader = claxon::FlacReader::open(path).expect("valid flac");
        let info = reader.streaminfo();
        let samples = reader.samples().map(|s| s.unwrap()).collect();
        (info, samples)
    }

    #[test]
    fn round_trips_through_an_independent_decoder() {
        let dir = tempfile_dir();
        let path = dir.join("t.flac");
        let input = sine(48_000 + 123, 2, 48_000.0); // not a multiple of the block size
        let mut w = FlacWriter::create(&path, 48_000, 2).unwrap();
        for chunk in input.chunks(960) {
            w.write(chunk).unwrap();
        }
        assert_eq!(w.finish().unwrap(), 48_123);
        let (info, decoded) = decode(&path);
        assert_eq!(info.sample_rate, 48_000);
        assert_eq!(info.channels, 2);
        assert_eq!(info.bits_per_sample, 16);
        assert_eq!(info.samples, Some(48_123));
        let expected: Vec<i32> = input
            .iter()
            .map(|s| (s.clamp(-1.0, 1.0) * 32767.0).round() as i32)
            .collect();
        assert_eq!(decoded, expected);
        assert_ne!(info.md5sum, [0u8; 16], "md5 written into the header");
    }

    #[test]
    fn empty_recording_is_still_a_valid_file() {
        let dir = tempfile_dir();
        let path = dir.join("empty.flac");
        FlacWriter::create(&path, 16_000, 1)
            .unwrap()
            .finish()
            .unwrap();
        let (info, decoded) = decode(&path);
        // FLAC stores 0 total samples as "unknown", which decoders report as None.
        assert!(matches!(info.samples, None | Some(0)));
        assert!(decoded.is_empty());
    }

    #[test]
    fn clips_out_of_range_samples() {
        let dir = tempfile_dir();
        let path = dir.join("clip.flac");
        let mut w = FlacWriter::create(&path, 16_000, 1).unwrap();
        w.write(&[2.0, -2.0, 0.0]).unwrap();
        w.finish().unwrap();
        let (_, decoded) = decode(&path);
        assert_eq!(decoded, [32767, -32767, 0]);
    }
}
