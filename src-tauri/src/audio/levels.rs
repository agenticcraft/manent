//! Audio level metering for the UI (RMS and peak in dBFS).

use std::time::Duration;

use serde::Serialize;

use super::sources::SourceKind;

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

/// How long a source may deliver nothing before it counts as having no input.
pub const NO_INPUT_AFTER: Duration = Duration::from_secs(3);

/// Notices a source that opened fine but never delivers anything (VRT-12). Without the user's
/// consent macOS gives system audio no samples or zeros, and a microphone zeros. A real
/// microphone is never exactly constant (its noise floor; measured: at least -76 dB peak to
/// peak per 50 ms), so a constant mic signal is nothing, like a dead digital mic stuck at one
/// value. An idle output also sends zeros, so for system audio only no samples count.
#[derive(Debug)]
pub struct InputWatch {
    /// A constant signal counts as nothing (microphones).
    constant_is_nothing: bool,
    waited: Duration,
    alive: bool,
}

impl InputWatch {
    pub fn new(kind: SourceKind) -> Self {
        Self {
            constant_is_nothing: kind == SourceKind::Mic,
            waited: Duration::ZERO,
            alive: false,
        }
    }

    /// Account one tick of `elapsed` in which `samples` arrived; true while there is no input.
    /// Once a source delivered, it has input (a later pause or silence is not this).
    pub fn tick(&mut self, samples: &[f32], elapsed: Duration) -> bool {
        let something = match samples.split_first() {
            None => false,
            Some((first, rest)) => !self.constant_is_nothing || rest.iter().any(|x| x != first),
        };
        self.alive |= something;
        if !self.alive {
            self.waited += elapsed;
        }
        !self.alive && self.waited >= NO_INPUT_AFTER
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_input_only_after_a_while_without_any_sample() {
        let tick = Duration::from_millis(50);
        let mut w = InputWatch::new(SourceKind::Loopback);
        let ticks = (NO_INPUT_AFTER.as_millis() / tick.as_millis()) as usize;
        assert!((1..ticks).all(|_| !w.tick(&[], tick)), "not yet");
        assert!(w.tick(&[], tick), "nothing for {NO_INPUT_AFTER:?}");
        assert!(!w.tick(&[0.1, -0.1], tick), "input arrived");
        assert!(!w.tick(&[], tick), "a later pause is not 'no input'");
    }

    #[test]
    fn zeros_are_system_audio_but_not_a_microphone() {
        let zeros = [0.0; 2400];
        let mut system = InputWatch::new(SourceKind::Loopback);
        assert!(!system.tick(&zeros, NO_INPUT_AFTER), "an idle output");
        let mut mic = InputWatch::new(SourceKind::Mic);
        assert!(mic.tick(&zeros, NO_INPUT_AFTER), "a mic without consent");
        assert!(
            !mic.tick(&[0.0, 1e-4, -1e-4], NO_INPUT_AFTER),
            "its noise floor"
        );
        assert!(
            !mic.tick(&zeros, NO_INPUT_AFTER),
            "silent later: still input"
        );
    }

    #[test]
    fn a_mic_stuck_at_one_value_has_no_input() {
        // The dead digital mic on the Linux test machine delivers a constant -0.177.
        let mut mic = InputWatch::new(SourceKind::Mic);
        assert!(mic.tick(&[-0.177; 2400], NO_INPUT_AFTER));
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
