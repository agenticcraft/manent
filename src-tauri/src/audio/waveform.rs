//! The waveform of a meeting for the player (VRT-20): loudness per bin over all tracks.

use std::path::Path;

use claxon::FlacReader;

/// RMS per bin of each track (bins spread over the longest track), the loudest track per bin,
/// scaled so the loud parts reach about 1 (the 98th percentile; a click doesn't flatten
/// everything else).
pub fn waveform(tracks: &[&Path], bins: usize) -> Result<Vec<f32>, claxon::Error> {
    let mut seconds = 0.0f64;
    for t in tracks {
        let info = FlacReader::open(t)?.streaminfo();
        seconds = seconds.max(info.samples.unwrap_or(0) as f64 / info.sample_rate as f64);
    }
    let mut out = vec![0.0f32; bins];
    if seconds <= 0.0 || bins == 0 {
        return Ok(out);
    }
    for t in tracks {
        let mut reader = FlacReader::open(t)?;
        let info = reader.streaminfo();
        let (rate, channels) = (info.sample_rate as f64, info.channels as usize);
        let scale = 1.0 / (1u64 << (info.bits_per_sample - 1)) as f64;
        let mut sums = vec![(0.0f64, 0u64); bins];
        let mut frame = 0u64;
        let mut acc = 0i64;
        for (i, s) in reader.samples().enumerate() {
            acc += i64::from(s?);
            if (i + 1) % channels == 0 {
                let v = acc as f64 / channels as f64 * scale;
                let bin = ((frame as f64 / rate / seconds) * bins as f64) as usize;
                let b = &mut sums[bin.min(bins - 1)];
                b.0 += v * v;
                b.1 += 1;
                frame += 1;
                acc = 0;
            }
        }
        for (o, (sum, n)) in out.iter_mut().zip(sums) {
            if n > 0 {
                *o = o.max((sum / n as f64).sqrt() as f32);
            }
        }
    }
    let mut sorted = out.clone();
    sorted.sort_by(f32::total_cmp);
    let top = sorted[(bins * 98 / 100).min(bins - 1)].max(f32::EPSILON);
    Ok(out.into_iter().map(|v| (v / top).min(1.0)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::flac::FlacWriter;

    #[test]
    fn loud_and_quiet_parts_show_over_the_longest_track() {
        let dir = crate::test_util::temp_dir();
        let (a, b) = (dir.join("mic.flac"), dir.join("system.flac"));
        // Track a: 1 s loud, 1 s silent. Track b: 4 s at a lower level.
        let mut w = FlacWriter::create(&a, 16_000, 1).unwrap();
        let loud: Vec<f32> = (0..16_000)
            .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
            .collect();
        w.write(&loud).unwrap();
        w.write(&vec![0.0; 16_000]).unwrap();
        w.finish().unwrap();
        let mut w = FlacWriter::create(&b, 48_000, 2).unwrap();
        w.write(&vec![0.1; 4 * 48_000 * 2]).unwrap();
        w.finish().unwrap();
        let wave = waveform(&[&a, &b], 8).unwrap();
        assert_eq!(wave.len(), 8);
        assert!(wave[0] > 0.9 && wave[1] > 0.9, "the loud second: {wave:?}");
        assert!(
            wave[2..].iter().all(|&v| v > 0.1 && v < 0.5),
            "the quieter track elsewhere: {wave:?}"
        );
    }
}
