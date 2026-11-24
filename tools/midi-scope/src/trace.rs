//! Fixed-rate sample buffer.
//!
//! Samples arrive at a constant rate, so a sample's position in the buffer *is*
//! its time. No timestamps, no expiry: the window is simply the buffer length,
//! and the span is `capacity / rate`.

use ringbuf::{
    HeapRb,
    traits::Observer,
};

use crate::acq::DataPoint;

/// Fixed-capacity history of values, oldest first.
///
/// A push never allocates and never scans; the oldest sample is dropped once
/// capacity is reached.
pub struct Trace {
    rb: HeapRb<DataPoint>,
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
    /// Creates a full buffer of zeros, so the scope draws a flat line at 0
    /// before any data arrives rather than an empty panel.
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity > 0, "trace capacity must be non-zero");
        let mut rb = HeapRb::new(capacity);
        while rb
            .try_push(DataPoint {
                value: 0.0,
                is_duplicate: false,
            })
            .is_ok()
        {}
        Self { rb }
    }

    /// Appends a value, overwriting the oldest one when full.
    pub fn push(&mut self, point: DataPoint) {
        self.rb.push_overwrite(point);
    }

    /// Values in chronological order, split across the ring's wrap point.
    pub fn slices(&mut self) -> (&[DataPoint], &[DataPoint]) {
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

    fn point(value: f32) -> DataPoint {
        DataPoint {
            value,
            is_duplicate: false,
        }
    }

    #[test]
    fn starts_prefilled_with_zeros() {
        let mut trace = Trace::with_capacity(8);
        assert_eq!(trace.len(), 8, "buffer starts full");
        let (first, second) = trace.slices();
        assert!(first.iter().chain(second).all(|p| p.value == 0.0));
    }

    #[test]
    fn pushing_after_prefill_replaces_the_oldest() {
        let mut trace = Trace::with_capacity(4);
        trace.push(point(5.0));
        assert_eq!(trace.len(), 4, "still full");
        let (first, second) = trace.slices();
        let values: Vec<_> = first.iter().chain(second).map(|p| p.value).collect();
        assert_eq!(values, vec![0.0, 0.0, 0.0, 5.0], "newest at the right edge");
    }

    #[test]
    fn keeps_values_in_order() {
        let mut trace = Trace::with_capacity(4);
        trace.push(point(64.0));
        trace.push(point(127.0));
        let (a, b) = trace.slices();
        assert_eq!(
            a.iter().chain(b).map(|p| p.value).collect::<Vec<_>>(),
            vec![0.0, 0.0, 64.0, 127.0],
            "newest values sit at the right edge"
        );
    }

    #[test]
    fn overwrites_the_oldest_when_full() {
        let mut trace = Trace::with_capacity(3);
        for value in 0..5 {
            trace.push(point(value as f32));
        }
        assert_eq!(trace.len(), 3, "capacity is a hard bound");
        let (a, b) = trace.slices();
        assert_eq!(a.iter().chain(b).map(|p| p.value).collect::<Vec<_>>(), vec![2.0, 3.0, 4.0]);
    }

    #[test]
    fn reads_do_not_change_the_trace() {
        let mut trace = Trace::with_capacity(4);
        trace.push(point(5.0));
        for _ in 0..1000 {
            let (a, b) = trace.slices();
            let _ = a.len() + b.len();
        }
        assert_eq!(trace.len(), 4);
    }

    #[test]
    fn capacity_is_fixed() {
        assert_eq!(Trace::with_capacity(2048).capacity(), 2048);
    }
}
