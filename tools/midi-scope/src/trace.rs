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

    /// Repeats the last value so the line holds flat between events.
    pub fn hold(&mut self, now: f64) {
        if let Some(&(_, value)) = self.samples.last() {
            self.samples.push((now, value));
            self.expire(now);
        }
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
    fn hold_extends_the_last_value() {
        let mut trace = Trace::new(10.0);
        trace.push(1.0, 99.0);
        trace.hold(2.0);
        assert_eq!(trace.samples(), &[(1.0, 99.0), (2.0, 99.0)]);
    }

    #[test]
    fn hold_on_empty_trace_does_nothing() {
        let mut trace = Trace::new(10.0);
        trace.hold(5.0);
        assert!(trace.is_empty());
    }

    #[test]
    fn empty_after_everything_expires() {
        let mut trace = Trace::new(1.0);
        trace.push(0.0, 5.0);
        trace.push(30.0, 6.0);
        assert_eq!(trace.samples(), &[(30.0, 6.0)]);
    }
}
