//! Audio level metering for the UI (RMS and peak in dBFS).

use serde::Serialize;

/// Lowest level reported; silence and digital zero map here.
pub const FLOOR_DB: f32 = -96.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Level {
    pub rms_db: f32,
    pub peak_db: f32,
}

fn to_db(v: f32) -> f32 {
    if v <= 0.0 {
        FLOOR_DB
    } else {
        (20.0 * v.log10()).max(FLOOR_DB)
    }
}

/// RMS and peak over a block of interleaved samples (all channels together). The RMS is
/// taken around the block's mean, so a constant DC offset (e.g. a dead digital microphone
/// stuck at one value) reads as silence instead of a healthy-looking level.
pub fn measure(samples: &[f32]) -> Level {
    if samples.is_empty() {
        return Level {
            rms_db: FLOOR_DB,
            peak_db: FLOOR_DB,
        };
    }
    let n = samples.len() as f64;
    let mean = samples.iter().map(|&x| x as f64).sum::<f64>() / n;
    let (sum_sq, peak) = samples.iter().fold((0.0f64, 0.0f32), |(s, p), &x| {
        let d = x as f64 - mean;
        (s + d * d, p.max(x.abs()))
    });
    let rms = (sum_sq / n).sqrt() as f32;
    Level {
        rms_db: to_db(rms),
        peak_db: to_db(peak),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_is_the_floor() {
        assert_eq!(measure(&[0.0; 100]).rms_db, FLOOR_DB);
        assert_eq!(measure(&[]).peak_db, FLOOR_DB);
    }

    #[test]
    fn full_scale_square_wave_is_zero_db() {
        let l = measure(&[1.0, -1.0, 1.0, -1.0]);
        assert!(l.rms_db.abs() < 1e-4 && l.peak_db.abs() < 1e-4, "{l:?}");
    }

    #[test]
    fn constant_dc_offset_reads_as_silence() {
        // A dead digital mic on the test machine delivers a constant -0.177.
        let l = measure(&[-0.177; 4800]);
        assert_eq!(l.rms_db, FLOOR_DB);
        assert!(
            (l.peak_db - -15.04).abs() < 0.05,
            "peak still shows the offset"
        );
        // A sine on top of the offset measures as the sine.
        let s: Vec<f32> = (0..4800)
            .map(|i| -0.177 + (i as f32 * std::f32::consts::TAU / 48.0).sin() * 0.5)
            .collect();
        assert!((measure(&s).rms_db - -9.03).abs() < 0.05);
    }

    #[test]
    fn sine_rms_is_about_3_db_below_peak() {
        let s: Vec<f32> = (0..4800)
            .map(|i| (i as f32 * std::f32::consts::TAU / 48.0).sin() * 0.5)
            .collect();
        let l = measure(&s);
        assert!((l.peak_db - -6.02).abs() < 0.05, "{l:?}");
        assert!((l.rms_db - -9.03).abs() < 0.05, "{l:?}");
    }
}
