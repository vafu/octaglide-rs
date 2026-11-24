//! Fixed-rate sample buffer.
//!
//! Samples arrive at a constant rate, so a sample's position in the buffer *is*
//! its time. No timestamps, no expiry: the window is simply the buffer length,
//! and the span is `capacity / rate`.

use ringbuf::{
    HeapRb,
    traits::Observer,
};

/// Fixed-capacity history of values, oldest first.
///
/// A push never allocates and never scans; the oldest sample is dropped once
/// capacity is reached.
pub struct Trace {
    rb: HeapRb<f32>,
}

impl std::fmt::Debug for Trace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Trace")
            .field("len", &self.len())
            .field("capacity", &self.capacity())
            .finish()
    }
}

impl Trace {
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity > 0, "trace capacity must be non-zero");
        Self {
            rb: HeapRb::new(capacity),
        }
    }

    /// Appends a value, overwriting the oldest one when full.
    pub fn push(&mut self, value: f32) {
        self.rb.push_overwrite(value);
    }

    /// Values in chronological order, split across the ring's wrap point.
    pub fn slices(&mut self) -> (&[f32], &[f32]) {
        self.rb.as_slices()
    }

    pub fn len(&self) -> usize {
        self.rb.occupied_len()
    }

    pub fn capacity(&self) -> usize {
        self.rb.capacity().get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_values_in_order() {
        let mut trace = Trace::with_capacity(8);
        trace.push(0.0);
        trace.push(64.0);
        trace.push(127.0);
        let (a, b) = trace.slices();
        assert_eq!(a.iter().chain(b).copied().collect::<Vec<_>>(), vec![0.0, 64.0, 127.0]);
    }

    #[test]
    fn overwrites_the_oldest_when_full() {
        let mut trace = Trace::with_capacity(3);
        for value in 0..5 {
            trace.push(value as f32);
        }
        assert_eq!(trace.len(), 3, "capacity is a hard bound");
        let (a, b) = trace.slices();
        assert_eq!(a.iter().chain(b).copied().collect::<Vec<_>>(), vec![2.0, 3.0, 4.0]);
    }

    #[test]
    fn reads_do_not_grow_the_trace() {
        let mut trace = Trace::with_capacity(4);
        trace.push(5.0);
        for _ in 0..1000 {
            let (a, b) = trace.slices();
            let _ = a.len() + b.len();
        }
        assert_eq!(trace.len(), 1);
    }

    #[test]
    fn empty_trace_has_no_slices() {
        let mut trace = Trace::with_capacity(4);
        assert_eq!(trace.len(), 0);
        assert_eq!(trace.slices(), (&[][..], &[][..]));
    }

    #[test]
    fn capacity_is_fixed() {
        assert_eq!(Trace::with_capacity(2048).capacity(), 2048);
    }
}
