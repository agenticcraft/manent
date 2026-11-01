//! Recovering a track left unfinished when the app died during a recording (VRT-41).
//!
//! The FLAC writer streams frames to the file and only rewrites the header (total samples,
//! frame sizes, MD5) on `finish`. After a crash or kill the header still has its placeholder
//! values, the last few kilobytes in the write buffer are lost and the last frame may be cut
//! off. Every complete frame before that is intact: [`repair_flac`] decodes those and writes
//! them again as a proper file.

use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::Path;

use super::flac::{BLOCK_SIZE, FlacError, FlacWriter};

#[derive(Debug, thiserror::Error)]
pub enum RecoverError {
    #[error("{0}: {1}")]
    Read(String, String),
    #[error(transparent)]
    Flac(#[from] FlacError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Rewrite the FLAC file at `path` from its complete frames, in place (through a temp file).
/// Returns the seconds kept. A file without a readable header can't be repaired.
pub fn repair_flac(path: &Path) -> Result<f64, RecoverError> {
    let name = path.display().to_string();
    let mut file = File::open(path)?;
    let mut head = [0u8; 42];
    file.read_exact(&mut head)
        .map_err(|e| RecoverError::Read(name.clone(), e.to_string()))?;
    fix_block_sizes(&mut head);
    let source = Cursor::new(head).chain(BufReader::new(file));
    let mut reader = claxon::FlacReader::new(source)
        .map_err(|e| RecoverError::Read(name.clone(), e.to_string()))?;
    let info = reader.streaminfo();
    let (rate, channels) = (info.sample_rate, info.channels as usize);
    let scale = 1.0 / f32::from(i16::MAX);
    let tmp = path.with_extension("flac.recovering");
    let mut writer = FlacWriter::create(&tmp, rate, channels as u16)?;
    let mut frames = reader.blocks();
    let mut buffer = Vec::new();
    let mut interleaved: Vec<f32> = Vec::new();
    // Stop at the first frame that can't be read: the cut-off end of the file.
    while let Ok(Some(block)) = frames.read_next_or_eof(std::mem::take(&mut buffer)) {
        interleaved.clear();
        for i in 0..block.duration() {
            for c in 0..channels {
                interleaved.push(block.sample(c as u32, i) as f32 * scale);
            }
        }
        writer.write(&interleaved)?;
        buffer = block.into_buffer();
    }
    let seconds = writer.seconds();
    writer.finish()?;
    std::fs::rename(&tmp, path)?;
    Ok(seconds)
}

/// Files recorded before VRT-41 have a placeholder header whose block sizes are invalid
/// (min > max), which decoders reject. Veritas always writes fixed-size blocks, so put those in.
/// `head` is the file's first 42 bytes: "fLaC", the STREAMINFO block header, then STREAMINFO,
/// which starts with the min and max block size (16 bits each).
fn fix_block_sizes(head: &mut [u8; 42]) {
    let min = u16::from_be_bytes([head[8], head[9]]);
    let max = u16::from_be_bytes([head[10], head[11]]);
    if &head[..4] == b"fLaC" && (min < 16 || min > max) {
        let size = (BLOCK_SIZE as u16).to_be_bytes();
        head[8..10].copy_from_slice(&size);
        head[10..12].copy_from_slice(&size);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::temp_dir;

    fn tone(frames: usize, channels: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| std::iter::repeat_n((i as f32 * 0.01).sin() * 0.4, channels))
            .collect()
    }

    /// What a kill leaves: frames written so far, the buffered tail lost, no final header.
    fn crashed_file(path: &Path, seconds: usize, rate: usize, channels: usize) -> Vec<f32> {
        let audio = tone(seconds * rate, channels);
        let mut w = FlacWriter::create(path, rate as u32, channels as u16).unwrap();
        for chunk in audio.chunks(rate / 20 * channels) {
            w.write(chunk).unwrap();
        }
        // No finish(), and no flush of the write buffer: as if the process died here.
        w.forget_unflushed();
        audio
    }

    #[test]
    fn a_killed_recording_is_repaired_up_to_its_last_complete_frame() {
        let dir = temp_dir();
        let path = dir.join("system.flac");
        let audio = crashed_file(&path, 30, 48_000, 2);
        let seconds = repair_flac(&path).unwrap();
        // Every complete block is kept; only the block still being collected is lost.
        let block = BLOCK_SIZE as f64 / 48_000.0;
        assert!(
            seconds > 30.0 - block && seconds <= 30.0,
            "kept {seconds} s of 30"
        );
        let mut reader = claxon::FlacReader::open(&path).expect("valid after repair");
        let info = reader.streaminfo();
        assert_eq!((info.sample_rate, info.channels), (48_000, 2));
        assert_eq!(info.samples, Some((seconds * 48_000.0).round() as u64));
        let decoded: Vec<i32> = reader.samples().map(|s| s.unwrap()).collect();
        let expected: Vec<i32> = audio[..decoded.len()]
            .iter()
            .map(|s| (s * 32767.0).round() as i32)
            .collect();
        assert_eq!(decoded, expected, "the kept audio is unchanged");
        assert!(!dir.join("system.flac.recovering").exists());
    }

    #[test]
    fn files_from_before_the_fix_with_an_invalid_placeholder_header_are_repaired_too() {
        let dir = temp_dir();
        let path = dir.join("old.flac");
        crashed_file(&path, 5, 16_000, 1);
        // The placeholder block sizes as they were written before VRT-41: min > max.
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[8..12].copy_from_slice(&[0xff, 0xff, 0x00, 0x00]);
        std::fs::write(&path, &bytes).unwrap();
        assert!(
            claxon::FlacReader::open(&path).is_err(),
            "unreadable as it is"
        );
        let seconds = repair_flac(&path).unwrap();
        let block = BLOCK_SIZE as f64 / 16_000.0;
        assert!(seconds > 5.0 - block && seconds <= 5.0, "kept {seconds} s");
        assert!(claxon::FlacReader::open(&path).is_ok());
    }

    #[test]
    fn a_crash_before_anything_was_flushed_is_an_error() {
        // The header sits in the write buffer too: the file is empty. Nothing to keep.
        let dir = temp_dir();
        let path = dir.join("mic.flac");
        crashed_file(&path, 0, 16_000, 1);
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 0);
        assert!(repair_flac(&path).is_err());
    }

    #[test]
    fn a_file_without_a_header_is_an_error_not_a_panic() {
        let dir = temp_dir();
        let path = dir.join("broken.flac");
        std::fs::write(&path, b"fLaC").unwrap();
        assert!(repair_flac(&path).is_err());
    }
}
