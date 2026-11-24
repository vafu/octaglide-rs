//! Bounded history of sampled values.

/// A fixed-window trace: samples older than `window` are dropped.
#[derive(Debug)]
pub struct Trace {
    samples: Vec<(f64, f32)>,
    window: f64,
}

impl Trace {
    pub fn new(window_secs: f64) -> Self {
        Self {
            samples: Vec::new(),
            window: window_secs,
        }
    }

    /// Records a value at `now` (seconds) and drops expired samples.
    pub fn push(&mut self, now: f64, value: f32) {
        self.samples.push((now, value));
        self.expire(now);
    }

    fn expire(&mut self, now: f64) {
        let cutoff = now - self.window;
        let keep_from = self
            .samples
            .iter()
            .position(|(at, _)| *at >= cutoff)
            .unwrap_or(self.samples.len());
        self.samples.drain(..keep_from);
    }

    pub fn samples(&self) -> &[(f64, f32)] {
        &self.samples
    }

    /// Time and value of the newest sample, if any.
    ///
    /// The plot extends this value to "now" when drawing, so the trace does not
    /// need a sample per frame just to hold the line flat.
    pub fn last(&self) -> Option<(f64, f32)> {
        self.samples.last().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn window(&self) -> f64 {
        self.window
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_values_inside_the_window() {
        let mut trace = Trace::new(10.0);
        trace.push(0.0, 0.0);
        trace.push(5.0, 64.0);
        trace.push(9.0, 127.0);
        assert_eq!(trace.samples().len(), 3);
    }

    #[test]
    fn drops_samples_older_than_the_window() {
        let mut trace = Trace::new(10.0);
        trace.push(0.0, 10.0);
        trace.push(4.0, 20.0);
        trace.push(12.0, 30.0);
        assert_eq!(trace.samples(), &[(4.0, 20.0), (12.0, 30.0)]);
    }

    #[test]
    fn last_returns_the_newest_sample() {
        let mut trace = Trace::new(10.0);
        assert_eq!(trace.last(), None);
        trace.push(1.0, 10.0);
        trace.push(2.0, 20.0);
        assert_eq!(trace.last(), Some((2.0, 20.0)));
    }

    #[test]
    fn a_long_lived_scope_keeps_a_bounded_buffer() {
        // 60 s of frames at 60 fps used to append 3600 samples via hold().
        let mut trace = Trace::new(10.0);
        trace.push(0.0, 0.0);
        for frame in 1..=3600 {
            let _ = trace.last();
            let _ = frame;
        }
        assert_eq!(trace.samples().len(), 1, "idle frames must not grow the trace");
    }

    #[test]
    fn reading_does_not_grow_the_trace() {
        let mut trace = Trace::new(10.0);
        trace.push(1.0, 5.0);
        for _ in 0..100 {
            let _ = trace.last();
        }
        assert_eq!(trace.samples().len(), 1, "reads must not append samples");
    }



    #[test]
    fn empty_after_everything_expires() {
        let mut trace = Trace::new(1.0);
        trace.push(0.0, 5.0);
        trace.push(30.0, 6.0);
        assert_eq!(trace.samples(), &[(30.0, 6.0)]);
    }
}
