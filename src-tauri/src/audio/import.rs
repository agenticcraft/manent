//! Imported recordings (VRT-19, decision-3): any file symphonia can read (wav, mp3, m4a/mp4/aac,
//! flac, ogg/vorbis, mkv/webm) is decoded once into a mono FLAC track at its own sample rate.
//! From then on the meeting plays and re-transcribes like a recording.

use std::fs::File;
use std::path::Path;

use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use super::flac::{FlacError, FlacWriter};

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("can't open {0}: {1}")]
    Open(String, std::io::Error),
    #[error("{0}: not an audio or video file Veritas can read")]
    Unsupported(String),
    #[error("{0}: has no audio")]
    NoAudio(String),
    #[error("{0}: {1}")]
    Decode(String, String),
    #[error(transparent)]
    Flac(#[from] FlacError),
}

/// Decode `src` into a mono FLAC file at `dest`. `progress` gets the fraction done when the
/// file's length is known. Returns the length in seconds.
pub fn import_to_flac(
    src: &Path,
    dest: &Path,
    progress: &mut dyn FnMut(f64),
) -> Result<f64, ImportError> {
    let name = src.file_name().map_or_else(
        || src.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let file = File::open(src).map_err(|e| ImportError::Open(name.clone(), e))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = src.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|_| ImportError::Unsupported(name.clone()))?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| ImportError::NoAudio(name.clone()))?;
    let track_id = track.id;
    let total_frames = track.num_frames;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| ImportError::NoAudio(name.clone()))?
        .clone();
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|_| ImportError::Unsupported(name.clone()))?;

    let mut writer: Option<FlacWriter> = None;
    let mut interleaved: Vec<f32> = Vec::new();
    let mut mono: Vec<f32> = Vec::new();
    let mut frames: u64 = 0;
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            // A truncated file ends here; keep what was decoded.
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(e) => return Err(ImportError::Decode(name, e.to_string())),
        };
        if packet.track_id != track_id {
            continue;
        }
        let buf = match decoder.decode(&packet) {
            Ok(b) => b,
            // One damaged packet is skipped, like a player does.
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(ImportError::Decode(name, e.to_string())),
        };
        let channels = buf.spec().channels().count().max(1);
        let rate = buf.spec().rate();
        interleaved.resize(buf.samples_interleaved(), 0.0);
        buf.copy_to_slice_interleaved(&mut interleaved);
        mono.clear();
        mono.extend(
            interleaved
                .chunks_exact(channels)
                .map(|f| f.iter().sum::<f32>() / channels as f32),
        );
        let w = match writer.as_mut() {
            Some(w) => w,
            None => writer.insert(FlacWriter::create(dest, rate, 1)?),
        };
        w.write(&mono)?;
        frames += mono.len() as u64;
        if let Some(total) = total_frames.filter(|t| *t > 0) {
            progress((frames as f64 / total as f64).min(1.0));
        }
    }
    let writer = writer.ok_or(ImportError::NoAudio(name))?;
    let seconds = writer.seconds();
    writer.finish()?;
    progress(1.0);
    Ok(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/import")
            .join(name)
    }

    /// Strength of `freq` (Goertzel) in mono samples at `rate`.
    fn tone(x: &[f32], rate: u32, freq: f32) -> f32 {
        let k = 2.0 * (std::f32::consts::TAU * freq / rate as f32).cos();
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for &v in x {
            let s = v + k * s1 - s2;
            s2 = s1;
            s1 = s;
        }
        (s1 * s1 + s2 * s2 - k * s1 * s2).max(0.0).sqrt() / x.len() as f32
    }

    #[test]
    fn decodes_every_offered_format_to_mono_flac() {
        for file in ["tone.wav", "tone.mp3", "tone.m4a", "tone.mp4", "tone.ogg"] {
            let dest = crate::test_util::temp_dir().join("import.flac");
            let mut last = 0.0;
            let seconds = import_to_flac(&fixture(file), &dest, &mut |p| last = p).unwrap();
            assert!((seconds - 2.0).abs() < 0.1, "{file}: {seconds} s");
            assert_eq!(last, 1.0, "{file}: progress reaches 1");
            let mut reader = claxon::FlacReader::open(&dest).unwrap();
            let info = reader.streaminfo();
            assert_eq!(info.channels, 1, "{file}: mono");
            let scale = 1.0 / (1u32 << (info.bits_per_sample - 1)) as f32;
            let x: Vec<f32> = reader
                .samples()
                .map(|s| s.unwrap() as f32 * scale)
                .collect();
            let mid = &x[x.len() / 4..x.len() * 3 / 4];
            let (at, off) = (
                tone(mid, info.sample_rate, 440.0),
                tone(mid, info.sample_rate, 1000.0),
            );
            assert!(
                at > 0.02 && at > 20.0 * off,
                "{file}: the 440 Hz tone survives ({at} vs {off})"
            );
        }
    }

    #[test]
    fn unreadable_files_get_a_clear_error() {
        let dir = crate::test_util::temp_dir();
        let text = dir.join("notes.mp3");
        std::fs::write(&text, b"this is not audio").unwrap();
        let err = import_to_flac(&text, &dir.join("x.flac"), &mut |_| {}).unwrap_err();
        assert!(matches!(err, ImportError::Unsupported(_)), "{err}");
        let err =
            import_to_flac(&dir.join("missing.wav"), &dir.join("x.flac"), &mut |_| {}).unwrap_err();
        assert!(matches!(err, ImportError::Open(..)), "{err}");
    }
}
