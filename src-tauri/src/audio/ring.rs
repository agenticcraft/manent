//! Lock-free sample ring between a realtime audio callback and a consumer thread.
//!
//! The writer side runs in the audio callback: it never blocks, locks or allocates. When the
//! ring is full the samples that don't fit are dropped and counted, so a slow consumer shows
//! up as a non-zero drop count instead of a glitching callback.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Create a ring holding `capacity` samples, split into a writer and a reader.
pub fn sample_ring(capacity: usize) -> (RingWriter, RingReader) {
    let (producer, consumer) = rtrb::RingBuffer::new(capacity);
    let dropped = Arc::new(AtomicU64::new(0));
    (
        RingWriter {
            producer,
            dropped: dropped.clone(),
        },
        RingReader { consumer, dropped },
    )
}

/// Realtime-safe writer, owned by the audio callback.
pub struct RingWriter {
    producer: rtrb::Producer<f32>,
    dropped: Arc<AtomicU64>,
}

impl RingWriter {
    /// Push as many samples as fit; returns how many were written. Never blocks.
    pub fn push(&mut self, samples: &[f32]) -> usize {
        let n = samples.len().min(self.producer.slots());
        if n > 0
            && let Ok(chunk) = self.producer.write_chunk_uninit(n)
        {
            chunk.fill_from_iter(samples[..n].iter().copied());
        }
        let lost = samples.len() - n;
        if lost > 0 {
            self.dropped.fetch_add(lost as u64, Ordering::Relaxed);
        }
        n
    }
}

/// Consumer side, owned by a recorder or transcription feed thread.
pub struct RingReader {
    consumer: rtrb::Consumer<f32>,
    dropped: Arc<AtomicU64>,
}

impl RingReader {
    /// Append all samples that are currently available to `out`; returns how many.
    pub fn drain_into(&mut self, out: &mut Vec<f32>) -> usize {
        let n = self.consumer.slots();
        if n == 0 {
            return 0;
        }
        let chunk = self
            .consumer
            .read_chunk(n)
            .expect("slots() samples are available");
        let (a, b) = chunk.as_slices();
        out.extend_from_slice(a);
        out.extend_from_slice(b);
        chunk.commit_all();
        n
    }

    /// Samples that didn't fit in the ring since it was created.
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_come_out_in_order() {
        let (mut w, mut r) = sample_ring(8);
        assert_eq!(w.push(&[1.0, 2.0, 3.0]), 3);
        let mut out = Vec::new();
        assert_eq!(r.drain_into(&mut out), 3);
        assert_eq!(out, [1.0, 2.0, 3.0]);
        assert_eq!(r.drain_into(&mut out), 0);
    }

    #[test]
    fn wraps_around_the_end_of_the_buffer() {
        let (mut w, mut r) = sample_ring(4);
        let mut out = Vec::new();
        w.push(&[1.0, 2.0, 3.0]);
        r.drain_into(&mut out);
        w.push(&[4.0, 5.0, 6.0]); // crosses the end of the 4-slot buffer
        out.clear();
        r.drain_into(&mut out);
        assert_eq!(out, [4.0, 5.0, 6.0]);
        assert_eq!(r.dropped(), 0);
    }

    #[test]
    fn overflow_drops_and_counts_instead_of_blocking() {
        let (mut w, mut r) = sample_ring(4);
        assert_eq!(w.push(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]), 4);
        assert_eq!(w.push(&[7.0]), 0);
        assert_eq!(r.dropped(), 3);
        let mut out = Vec::new();
        r.drain_into(&mut out);
        assert_eq!(out, [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn writer_and_reader_work_across_threads() {
        let (mut w, mut r) = sample_ring(1024);
        let producer = std::thread::spawn(move || {
            for i in 0..100 {
                let block = [i as f32; 10];
                while w.push(&block) == 0 {
                    std::thread::yield_now();
                }
            }
        });
        producer.join().unwrap();
        let mut out = Vec::new();
        r.drain_into(&mut out);
        assert_eq!(out.len(), 1000);
        assert_eq!(out[999], 99.0);
    }
}
