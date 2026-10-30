//! Streaming mono resampler (rubato sinc, fixed input chunks). Feed any number of samples,
//! get resampled samples back; `finish` flushes the tail. The resampler's delay is removed,
//! so the output lines up with the input and has length ≈ input × ratio.

use audioadapter_buffers::direct::InterleavedSlice;
use rubato::{
    Async, FixedAsync, Indexing, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

const CHUNK: usize = 1024;

pub struct StreamResampler {
    inner: Option<Async<f32>>,
    ratio: f64,
    pending: Vec<f32>,
    out: Vec<f32>,
    /// Output frames still to drop to compensate the resampler delay.
    skip: usize,
    consumed: u64,
    produced: u64,
}

impl StreamResampler {
    pub fn new(from_rate: u32, to_rate: u32) -> Self {
        let ratio = to_rate as f64 / from_rate as f64;
        let inner = (from_rate != to_rate).then(|| {
            let params = SincInterpolationParameters::new(128, WindowFunction::Blackman2)
                .oversampling_factor(256)
                .interpolation(SincInterpolationType::Linear);
            Async::<f32>::new_sinc(ratio, 1.0, &params, CHUNK, 1, FixedAsync::Input)
                .expect("valid resampler parameters")
        });
        let skip = inner.as_ref().map_or(0, |r| r.output_delay());
        Self {
            inner,
            ratio,
            pending: Vec::new(),
            out: vec![0.0; CHUNK * 4],
            skip,
            consumed: 0,
            produced: 0,
        }
    }

    /// Resample `input`, appending the output to `dest`.
    pub fn process(&mut self, input: &[f32], dest: &mut Vec<f32>) {
        self.consumed += input.len() as u64;
        let Some(r) = self.inner.as_mut() else {
            dest.extend_from_slice(input);
            self.produced += input.len() as u64;
            return;
        };
        self.pending.extend_from_slice(input);
        let mut start = 0;
        while self.pending.len() - start >= r.input_frames_next() {
            let need = r.input_frames_next();
            let written = run(r, &self.pending[start..start + need], &mut self.out);
            start += need;
            push_skipping(
                &self.out[..written],
                &mut self.skip,
                dest,
                &mut self.produced,
            );
        }
        self.pending.drain(..start);
    }

    /// Flush the remaining input (zero-padded) and trim the output to the expected length.
    pub fn finish(mut self, dest: &mut Vec<f32>) {
        let Some(mut r) = self.inner.take() else {
            return;
        };
        let expected = (self.consumed as f64 * self.ratio).round() as u64;
        // Feed silence until all real input (plus the delay) has come out.
        while self.produced < expected {
            let need = r.input_frames_next();
            self.pending.resize(need.max(self.pending.len()), 0.0);
            let chunk: Vec<f32> = self.pending.drain(..need).collect();
            let written = run(&mut r, &chunk, &mut self.out);
            let room = (expected - self.produced) as usize;
            let mut tmp = Vec::new();
            push_skipping(&self.out[..written], &mut self.skip, &mut tmp, &mut 0);
            tmp.truncate(room);
            self.produced += tmp.len() as u64;
            dest.extend_from_slice(&tmp);
        }
    }
}

fn run(r: &mut Async<f32>, input: &[f32], out: &mut Vec<f32>) -> usize {
    let max = r.output_frames_max();
    if out.len() < max {
        out.resize(max, 0.0);
    }
    let input_adapter = InterleavedSlice::new(input, 1, input.len()).expect("mono input");
    let cap = out.len();
    let mut output_adapter = InterleavedSlice::new_mut(out, 1, cap).expect("mono output");
    let (_, written) = r
        .process_into_buffer(&input_adapter, &mut output_adapter, Some(&Indexing::new()))
        .expect("resampling a full chunk");
    written
}

fn push_skipping(samples: &[f32], skip: &mut usize, dest: &mut Vec<f32>, produced: &mut u64) {
    let drop = (*skip).min(samples.len());
    *skip -= drop;
    dest.extend_from_slice(&samples[drop..]);
    *produced += (samples.len() - drop) as u64;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f32, rate: u32, seconds: f32) -> Vec<f32> {
        (0..(rate as f32 * seconds) as usize)
            .map(|i| (i as f32 * freq * std::f32::consts::TAU / rate as f32).sin() * 0.5)
            .collect()
    }

    /// Strength of `freq` in `x` (Goertzel), normalised so a 0.5 sine gives ~0.25.
    pub(crate) fn goertzel(x: &[f32], freq: f32, rate: u32) -> f32 {
        let k = 2.0 * (std::f32::consts::TAU * freq / rate as f32).cos();
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for &v in x {
            let s = v + k * s1 - s2;
            s2 = s1;
            s1 = s;
        }
        (s1 * s1 + s2 * s2 - k * s1 * s2).max(0.0).sqrt() / x.len() as f32
    }

    fn resample_all(input: &[f32], from: u32, to: u32, chunk: usize) -> Vec<f32> {
        let mut r = StreamResampler::new(from, to);
        let mut out = Vec::new();
        for c in input.chunks(chunk) {
            r.process(c, &mut out);
        }
        r.finish(&mut out);
        out
    }

    #[test]
    fn same_rate_passes_through() {
        let x = tone(440.0, 48_000, 0.1);
        assert_eq!(resample_all(&x, 48_000, 48_000, 333), x);
    }

    #[test]
    fn upsampling_keeps_length_and_frequency() {
        let x = tone(1000.0, 44_100, 1.0);
        let y = resample_all(&x, 44_100, 48_000, 777);
        assert_eq!(y.len(), 48_000);
        let mid = &y[12_000..36_000];
        assert!(goertzel(mid, 1000.0, 48_000) > 0.2, "tone kept");
        assert!(
            goertzel(mid, 900.0, 48_000) < 0.02,
            "no smearing to other frequencies"
        );
    }

    #[test]
    fn downsampling_to_16k_for_transcription() {
        let x = tone(440.0, 48_000, 2.0);
        let y = resample_all(&x, 48_000, 16_000, 960);
        assert_eq!(y.len(), 32_000);
        assert!(goertzel(&y[4_000..28_000], 440.0, 16_000) > 0.2);
    }

    #[test]
    fn output_is_aligned_with_the_input() {
        // An impulse at 0.5 s must come out at 0.5 s (delay compensated).
        let mut x = vec![0.0f32; 44_100];
        x[22_050] = 1.0;
        let y = resample_all(&x, 44_100, 48_000, 1000);
        let peak = y
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .unwrap()
            .0;
        assert!(
            (peak as i64 - 24_000).abs() <= 2,
            "impulse at {peak}, expected 24000"
        );
    }

    #[test]
    fn chunking_does_not_change_the_result() {
        let x = tone(300.0, 44_100, 0.5);
        let a = resample_all(&x, 44_100, 48_000, 100);
        let b = resample_all(&x, 44_100, 48_000, 5000);
        assert_eq!(a.len(), b.len());
        let max_diff = a
            .iter()
            .zip(&b)
            .map(|(p, q)| (p - q).abs())
            .fold(0.0, f32::max);
        assert!(max_diff < 1e-5, "{max_diff}");
    }
}
