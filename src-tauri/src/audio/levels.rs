//! Audio level metering for the UI (RMS and peak in dBFS).

use std::time::Duration;

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

/// How long a source may deliver nothing at all before it counts as having no input.
pub const NO_INPUT_AFTER: Duration = Duration::from_secs(3);

/// Notices a source that opened fine but never delivers a sample (VRT-12): on macOS that is
/// what system audio does without the user's consent, and a dead device looks the same.
/// Digital silence is input (an idle output still sends zeros), so only no samples count.
#[derive(Debug, Default)]
pub struct InputWatch {
    waited: Duration,
    received: bool,
}

impl InputWatch {
    /// Account one tick of `elapsed` in which `samples` arrived; true while there is no input.
    pub fn tick(&mut self, samples: usize, elapsed: Duration) -> bool {
        self.received |= samples > 0;
        if !self.received {
            self.waited += elapsed;
        }
        !self.received && self.waited >= NO_INPUT_AFTER
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_input_only_after_a_while_without_any_sample() {
        let tick = Duration::from_millis(50);
        let mut w = InputWatch::default();
        let ticks = (NO_INPUT_AFTER.as_millis() / tick.as_millis()) as usize;
        assert!((1..ticks).all(|_| !w.tick(0, tick)), "not yet");
        assert!(w.tick(0, tick), "nothing for {NO_INPUT_AFTER:?}");
        assert!(!w.tick(480, tick), "input arrived");
        assert!(!w.tick(0, tick), "a later pause is not 'no input'");
    }

    #[test]
    fn digital_silence_is_input() {
        let mut w = InputWatch::default();
        assert!(!w.tick(4800, NO_INPUT_AFTER));
        assert!(!w.tick(0, NO_INPUT_AFTER));
    }

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
