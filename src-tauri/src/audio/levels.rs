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

/// RMS and peak over a block of interleaved samples (all channels together).
pub fn measure(samples: &[f32]) -> Level {
    if samples.is_empty() {
        return Level {
            rms_db: FLOOR_DB,
            peak_db: FLOOR_DB,
        };
    }
    let (sum_sq, peak) = samples.iter().fold((0.0f64, 0.0f32), |(s, p), &x| {
        (s + (x as f64) * (x as f64), p.max(x.abs()))
    });
    let rms = (sum_sq / samples.len() as f64).sqrt() as f32;
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
    fn sine_rms_is_about_3_db_below_peak() {
        let s: Vec<f32> = (0..4800)
            .map(|i| (i as f32 * std::f32::consts::TAU / 48.0).sin() * 0.5)
            .collect();
        let l = measure(&s);
        assert!((l.peak_db - -6.02).abs() < 0.05, "{l:?}");
        assert!((l.rms_db - -9.03).abs() < 0.05, "{l:?}");
    }
}
