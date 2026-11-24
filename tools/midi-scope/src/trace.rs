//! Bounded history of sampled values, backed by `ringbuf`.

use ringbuf::{
    HeapRb,
    traits::{Observer, RingBuffer},
};

/// Fixed-capacity trace of `(seconds, value)` samples, oldest first.
///
/// A push never allocates and never scans; the oldest sample is dropped once
/// capacity is reached, so memory is bounded by capacity rather than runtime.
pub struct Trace {
    rb: HeapRb<(f64, f32)>,
    window: f64,
}

impl std::fmt::Debug for Trace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Trace")
            .field("len", &self.len())
            .field("capacity", &self.capacity())
            .field("window", &self.window)
            .finish()
    }
}

impl Trace {
    pub fn with_capacity(capacity: usize, window_secs: f64) -> Self {
        assert!(capacity > 0, "trace capacity must be non-zero");
        Self {
            rb: HeapRb::new(capacity),
            window: window_secs,
        }
    }

    /// Appends a sample, overwriting the oldest one when full.
    pub fn push(&mut self, now: f64, value: f32) {
        self.rb.push_overwrite((now, value));
    }

    /// Samples in chronological order, split across the ring's wrap point.
    pub fn slices(&mut self) -> (&[(f64, f32)], &[(f64, f32)]) {
        self.rb.as_slices()
    }

    /// Samples in chronological order.
    pub fn iter(&mut self) -> impl Iterator<Item = (f64, f32)> + '_ {
        let (first, second) = self.rb.as_slices();
        first.iter().chain(second).copied()
    }

    pub fn last(&mut self) -> Option<(f64, f32)> {
        self.rb.last().copied()
    }

    pub fn len(&self) -> usize {
        self.rb.occupied_len()
    }

    pub fn is_empty(&self) -> bool {
        self.rb.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.rb.capacity().get()
    }

    pub fn window(&self) -> f64 {
        self.window
    }

    /// Drops samples older than the window, keeping the newest one.
    pub fn expire(&mut self, now: f64) {
        let cutoff = now - self.window;
        // Count how many leading samples are stale, then drop them in one go.
        let stale = self
            .rb
            .iter()
            .take_while(|(at, _)| *at < cutoff)
            .count()
            .min(self.len().saturating_sub(1));
        if stale > 0 {
            self.rb.skip(stale);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_values_in_order() {
        let mut trace = Trace::with_capacity(8, 10.0);
        trace.push(0.0, 0.0);
        trace.push(5.0, 64.0);
        trace.push(9.0, 127.0);
        let values: Vec<_> = trace.iter().collect();
        assert_eq!(values, vec![(0.0, 0.0), (5.0, 64.0), (9.0, 127.0)]);
    }

    #[test]
    fn overwrites_the_oldest_when_full() {
        let mut trace = Trace::with_capacity(3, 10.0);
        for index in 0..5 {
            trace.push(index as f64, index as f32);
        }
        assert_eq!(trace.len(), 3, "capacity is a hard bound");
        let values: Vec<_> = trace.iter().map(|(_, value)| value).collect();
        assert_eq!(values, vec![2.0, 3.0, 4.0], "oldest samples were dropped");
    }

    #[test]
    fn last_returns_the_newest_sample() {
        let mut trace = Trace::with_capacity(4, 10.0);
        assert_eq!(trace.last(), None);
        trace.push(1.0, 10.0);
        trace.push(2.0, 20.0);
        assert_eq!(trace.last(), Some((2.0, 20.0)));
    }

    #[test]
    fn reads_do_not_grow_the_trace() {
        let mut trace = Trace::with_capacity(4, 10.0);
        trace.push(1.0, 5.0);
        for _ in 0..1000 {
            let _ = trace.last();
            let _ = trace.iter().count();
        }
        assert_eq!(trace.len(), 1, "reads must not append samples");
    }

    #[test]
    fn expire_drops_samples_older_than_the_window() {
        let mut trace = Trace::with_capacity(8, 10.0);
        trace.push(0.0, 10.0);
        trace.push(4.0, 20.0);
        trace.push(12.0, 30.0);
        trace.expire(12.0);
        let values: Vec<_> = trace.iter().map(|(_, value)| value).collect();
        assert_eq!(values, vec![20.0, 30.0]);
    }

    #[test]
    fn expire_keeps_the_newest_sample() {
        let mut trace = Trace::with_capacity(4, 1.0);
        trace.push(0.0, 42.0);
        trace.expire(100.0);
        assert_eq!(trace.last(), Some((0.0, 42.0)), "never empty out completely");
    }

    #[test]
    fn empty_trace_is_empty() {
        let mut trace = Trace::with_capacity(4, 10.0);
        assert!(trace.is_empty());
        assert_eq!(trace.slices(), (&[][..], &[][..]));
    }

    #[test]
    fn capacity_is_fixed() {
        let trace = Trace::with_capacity(2048, 10.0);
        assert_eq!(trace.capacity(), 2048);
    }
}
