//! Mixdown of a meeting's tracks into one small playback file: mono 48 kHz Ogg Opus.
//!
//! Streaming: the FLAC tracks are decoded block by block, downmixed to mono, resampled to
//! 48 kHz if needed and summed, then encoded in 20 ms Opus frames, so an hour-long meeting
//! never sits in memory. The container follows RFC 7845 (OpusHead, OpusTags, granule
//! positions at 48 kHz, end-of-stream page).

use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

use ogg::writing::PacketWriteEndInfo;

use super::resample::StreamResampler;

const RATE: u32 = 48_000;
const FRAME: usize = 960; // 20 ms at 48 kHz
const BITRATE: i32 = 32_000;
const SERIAL: u32 = 0x5645_5249; // "VERI"
/// End an Ogg page every second of audio (50 × 20 ms): players seek by bisecting on page
/// granule positions, so one huge page would make the file unseekable.
const PACKETS_PER_PAGE: u64 = 50;

#[derive(Debug, thiserror::Error)]
pub enum MixError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("reading {0}: {1}")]
    Flac(PathBuf, claxon::Error),
    #[error("opus: {0}")]
    Opus(#[from] opus::Error),
    #[error("no tracks to mix")]
    NoTracks,
}

/// What the mixdown produced.
#[derive(Debug, Clone, PartialEq)]
pub struct MixSummary {
    pub seconds: f64,
    pub bytes: u64,
}

/// One input track, decoded and resampled on demand.
struct Input {
    path: PathBuf,
    reader: claxon::FlacReader<BufReader<File>>,
    resampler: Option<StreamResampler>,
    channels: u32,
    scale: f32,
    block: Vec<i32>,
    mono: Vec<f32>,
    queue: VecDeque<f32>,
    done: bool,
}

impl Input {
    fn open(path: &Path, rate: u32) -> Result<Self, MixError> {
        let file = BufReader::new(File::open(path)?);
        let reader =
            claxon::FlacReader::new(file).map_err(|e| MixError::Flac(path.to_owned(), e))?;
        let info = reader.streaminfo();
        Ok(Self {
            path: path.to_owned(),
            resampler: Some(StreamResampler::new(info.sample_rate, rate)),
            channels: info.channels,
            scale: 1.0 / (1u32 << (info.bits_per_sample - 1)) as f32,
            reader,
            block: Vec::new(),
            mono: Vec::new(),
            queue: VecDeque::new(),
            done: false,
        })
    }

    /// Decode one more FLAC block into the resampled mono queue (or flush at the end).
    fn fill(&mut self) -> Result<(), MixError> {
        let buffer = std::mem::take(&mut self.block);
        let next = self
            .reader
            .blocks()
            .read_next_or_eof(buffer)
            .map_err(|e| MixError::Flac(self.path.clone(), e))?;
        let mut out = Vec::new();
        match next {
            Some(block) => {
                self.mono.clear();
                let n = block.duration();
                self.mono.extend((0..n).map(|i| {
                    let sum: i64 = (0..self.channels).map(|c| block.sample(c, i) as i64).sum();
                    sum as f32 * self.scale / self.channels as f32
                }));
                if let Some(r) = self.resampler.as_mut() {
                    r.process(&self.mono, &mut out);
                }
                self.block = block.into_buffer();
            }
            None => {
                if let Some(r) = self.resampler.take() {
                    r.finish(&mut out);
                }
                self.done = true;
            }
        }
        self.queue.extend(out);
        Ok(())
    }
}

/// A whole FLAC track as mono samples at `rate` (the final transcription pass, VRT-17).
pub fn decode_mono(path: &Path, rate: u32) -> Result<Vec<f32>, MixError> {
    let mut input = Input::open(path, rate)?;
    while !input.done {
        input.fill()?;
    }
    Ok(input.queue.into_iter().collect())
}

/// Mix `tracks` into `out` (Ogg Opus, mono, 48 kHz).
pub fn mixdown(tracks: &[PathBuf], out: &Path) -> Result<MixSummary, MixError> {
    if tracks.is_empty() {
        return Err(MixError::NoTracks);
    }
    let mut inputs = tracks
        .iter()
        .map(|p| Input::open(p, RATE))
        .collect::<Result<Vec<_>, _>>()?;

    let mut encoder = opus::Encoder::new(RATE, opus::Channels::Mono, opus::Application::Voip)?;
    encoder.set_bitrate(opus::Bitrate::Bits(BITRATE))?;
    encoder.set_signal(opus::Signal::Voice)?;
    let pre_skip = encoder.get_lookahead()? as u64;

    let mut writer = ogg::PacketWriter::new(BufWriter::new(File::create(out)?));
    writer.write_packet(
        opus_head(pre_skip as u16),
        SERIAL,
        PacketWriteEndInfo::EndPage,
        0,
    )?;
    writer.write_packet(opus_tags(), SERIAL, PacketWriteEndInfo::EndPage, 0)?;

    let mut frame = [0.0f32; FRAME];
    let mut packet = vec![0u8; 4000];
    let mut total: u64 = 0; // real (unpadded) samples mixed
    let mut encoded: u64 = 0; // samples fed to the encoder, including padding
    let mut pending: Option<Vec<u8>> = None;

    loop {
        for input in inputs.iter_mut() {
            while !input.done && input.queue.len() < FRAME {
                input.fill()?;
            }
        }
        let available = inputs.iter().map(|i| i.queue.len()).max().unwrap_or(0);
        if available == 0 {
            break;
        }
        let n = available.min(FRAME);
        frame.fill(0.0);
        for input in inputs.iter_mut() {
            let take = n.min(input.queue.len());
            for (slot, v) in frame.iter_mut().zip(input.queue.drain(..take)) {
                *slot += v;
            }
        }
        for v in &mut frame {
            *v = v.clamp(-1.0, 1.0);
        }
        total += n as u64;
        if let Some(p) = pending.take() {
            writer.write_packet(p, SERIAL, page_end(encoded), pre_skip + encoded)?;
        }
        let len = encoder.encode_float(&frame, &mut packet)?;
        encoded += FRAME as u64;
        pending = Some(packet[..len].to_vec());
    }

    // Push the encoder's lookahead out with silence so the last real samples are included.
    let silence = [0.0f32; FRAME];
    while encoded < total + pre_skip {
        if let Some(p) = pending.take() {
            writer.write_packet(p, SERIAL, page_end(encoded), pre_skip + encoded)?;
        }
        let len = encoder.encode_float(&silence, &mut packet)?;
        encoded += FRAME as u64;
        pending = Some(packet[..len].to_vec());
    }
    // The last packet ends the stream; its granule position trims the padding.
    if let Some(p) = pending.take() {
        writer.write_packet(p, SERIAL, PacketWriteEndInfo::EndStream, pre_skip + total)?;
    }
    drop(writer);
    Ok(MixSummary {
        seconds: total as f64 / RATE as f64,
        bytes: std::fs::metadata(out)?.len(),
    })
}

/// Close the page after every `PACKETS_PER_PAGE` packets (`encoded` = samples written so far).
fn page_end(encoded: u64) -> PacketWriteEndInfo {
    if (encoded / FRAME as u64).is_multiple_of(PACKETS_PER_PAGE) {
        PacketWriteEndInfo::EndPage
    } else {
        PacketWriteEndInfo::NormalPacket
    }
}

/// RFC 7845 identification header for a mono stream.
fn opus_head(pre_skip: u16) -> Vec<u8> {
    let mut h = b"OpusHead".to_vec();
    h.push(1); // version
    h.push(1); // channels
    h.extend_from_slice(&pre_skip.to_le_bytes());
    h.extend_from_slice(&RATE.to_le_bytes()); // original input rate (informational)
    h.extend_from_slice(&0i16.to_le_bytes()); // output gain
    h.push(0); // channel mapping family 0
    h
}

/// RFC 7845 comment header (vendor string, no comments).
fn opus_tags() -> Vec<u8> {
    let vendor = b"Veritas";
    let mut t = b"OpusTags".to_vec();
    t.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    t.extend_from_slice(vendor);
    t.extend_from_slice(&0u32.to_le_bytes());
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::flac::FlacWriter;
    use crate::test_util::temp_dir;

    fn write_tone(path: &Path, freq: f32, rate: u32, channels: u16, seconds: f32) {
        let mut w = FlacWriter::create(path, rate, channels).unwrap();
        let samples: Vec<f32> = (0..(rate as f32 * seconds) as usize)
            .flat_map(|i| {
                let v = (i as f32 * freq * std::f32::consts::TAU / rate as f32).sin() * 0.3;
                std::iter::repeat_n(v, channels as usize)
            })
            .collect();
        w.write(&samples).unwrap();
        w.finish().unwrap();
    }

    /// Decode an Ogg Opus file with libopus; returns (OpusHead bytes, pcm after pre-skip,
    /// last granule position).
    fn decode(path: &Path) -> (Vec<u8>, Vec<f32>, u64) {
        let mut reader = ogg::PacketReader::new(BufReader::new(File::open(path).unwrap()));
        let head = reader.read_packet().unwrap().unwrap().data;
        let tags = reader.read_packet().unwrap().unwrap();
        assert!(tags.data.starts_with(b"OpusTags"));
        let pre_skip = u16::from_le_bytes([head[10], head[11]]) as usize;
        let mut decoder = opus::Decoder::new(48_000, opus::Channels::Mono).unwrap();
        let mut pcm = Vec::new();
        let mut buf = vec![0f32; 5760];
        let mut last_granule = 0;
        while let Some(p) = reader.read_packet().unwrap() {
            let n = decoder.decode_float(&p.data, &mut buf, false).unwrap();
            pcm.extend_from_slice(&buf[..n]);
            last_granule = p.absgp_page();
            if p.last_in_stream() {
                break;
            }
        }
        let end = (last_granule as usize).min(pcm.len());
        (head, pcm[pre_skip..end].to_vec(), last_granule)
    }

    fn strength(x: &[f32], freq: f32) -> f32 {
        let k = 2.0 * (std::f32::consts::TAU * freq / 48_000.0).cos();
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for &v in x {
            let s = v + k * s1 - s2;
            s2 = s1;
            s1 = s;
        }
        (s1 * s1 + s2 * s2 - k * s1 * s2).max(0.0).sqrt() / x.len() as f32
    }

    #[test]
    fn mixes_tracks_of_different_rates_into_valid_ogg_opus() {
        let dir = temp_dir();
        let mic = dir.join("mic.flac");
        let system = dir.join("system.flac");
        write_tone(&mic, 440.0, 48_000, 2, 2.0);
        write_tone(&system, 1500.0, 44_100, 1, 2.0);
        let out = dir.join("mix.opus");
        let summary = mixdown(&[mic, system], &out).unwrap();
        assert!((summary.seconds - 2.0).abs() < 0.01, "{summary:?}");
        assert!(
            summary.bytes < 2 * 32_000 / 8 + 4_000,
            "~32 kbps: {} bytes",
            summary.bytes
        );

        let (head, pcm, _) = decode(&out);
        assert_eq!(&head[..8], b"OpusHead");
        assert_eq!(head[9], 1, "mono");
        assert_eq!(
            pcm.len(),
            96_000,
            "granule positions trim to the exact length"
        );
        let mid = &pcm[24_000..72_000];
        assert!(strength(mid, 440.0) > 0.08, "mic tone in the mix");
        assert!(
            strength(mid, 1500.0) > 0.08,
            "system tone (resampled from 44.1 kHz) in the mix"
        );
        assert!(strength(mid, 1000.0) < 0.01, "nothing in between");
    }

    #[test]
    fn pages_end_every_second_for_seeking() {
        let dir = temp_dir();
        let a = dir.join("a.flac");
        write_tone(&a, 440.0, 48_000, 1, 5.0);
        let out = dir.join("mix.opus");
        mixdown(&[a], &out).unwrap();
        let mut reader = ogg::PacketReader::new(BufReader::new(File::open(&out).unwrap()));
        let mut granules = Vec::new();
        while let Some(p) = reader.read_packet().unwrap() {
            if p.last_in_page() {
                granules.push(p.absgp_page());
            }
        }
        // 2 header pages + about one audio page per second.
        assert!(
            (7..=9).contains(&granules.len()),
            "{} pages: {granules:?}",
            granules.len()
        );
        let audio: Vec<_> = granules[2..].to_vec();
        // Non-decreasing (RFC 7845): the final page only holds the trimmed encoder padding.
        assert!(
            audio.windows(2).all(|w| w[1] >= w[0]),
            "granules: {audio:?}"
        );
        assert!(
            audio.windows(2).all(|w| w[1] - w[0] <= 48_000),
            "≤ 1 s per page: {audio:?}"
        );
    }

    #[test]
    fn shorter_track_is_padded_with_silence() {
        let dir = temp_dir();
        let a = dir.join("a.flac");
        let b = dir.join("b.flac");
        write_tone(&a, 440.0, 48_000, 1, 1.0);
        write_tone(&b, 440.0, 48_000, 1, 0.5);
        let out = dir.join("mix.opus");
        let summary = mixdown(&[a, b], &out).unwrap();
        assert!((summary.seconds - 1.0).abs() < 0.01);
        let (_, pcm, _) = decode(&out);
        assert_eq!(pcm.len(), 48_000);
    }

    #[test]
    fn decodes_a_track_to_mono_at_another_rate() {
        let dir = crate::test_util::temp_dir();
        let path = dir.join("system.flac");
        write_tone(&path, 440.0, 48_000, 2, 2.0);
        let x = decode_mono(&path, 16_000).unwrap();
        assert_eq!(x.len(), 32_000, "2 s at 16 kHz");
        // strength() assumes 48 kHz: at 16 kHz the 440 Hz tone sits at 3 x 440 in its terms.
        let tone = strength(&x[8000..24000], 3.0 * 440.0);
        assert!(tone > 0.1, "440 Hz kept ({tone})");
        assert!(strength(&x[8000..24000], 3.0 * 1000.0) < 0.01);
    }

    #[test]
    fn no_tracks_is_an_error() {
        let dir = temp_dir();
        assert!(matches!(
            mixdown(&[], &dir.join("x.opus")),
            Err(MixError::NoTracks)
        ));
    }
}
