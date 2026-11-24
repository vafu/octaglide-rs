//! Acquisition stage: samples the latest MIDI value on a fixed clock.
//!
//! The scope is not driven by MIDI timing. MIDI events land in ring A; this
//! stage reads the newest of them and emits one point every `SAMPLE_PERIOD`
//! into ring B. The UI drains ring B. A silent input therefore still produces a
//! flat, correctly-timed trace.

use std::time::Duration;

use ringbuf::{
    HeapCons, HeapProd,
    traits::{Consumer, Producer, Split},
};

/// Samples per second emitted to the display ring.
pub const SAMPLE_HZ: u32 = 250;

/// Wall-clock spacing between emitted samples.
pub const SAMPLE_PERIOD: Duration = Duration::from_micros(1_000_000 / SAMPLE_HZ as u64);

/// Visible time span, in seconds.
pub const WINDOW_SECS: f64 = 10.0;

/// Display ring capacity: enough for the whole window at `SAMPLE_HZ`.
pub const DISPLAY_CAPACITY: usize = (SAMPLE_HZ as usize * WINDOW_SECS as usize) + 64;

/// Ring A capacity: one second of incoming events, so a stalled acquisition
/// stage drops instead of blocking the MIDI callback.
pub const INBOX_CAPACITY: usize = 1024;

/// One sampled point: seconds since start, value.
pub type Sample = (f64, f32);

pub struct Acquisition {
    inbox: HeapCons<Sample>,
    outbox: HeapProd<Sample>,
    latest: f32,
    has_value: bool,
}

impl Acquisition {
    pub fn new(inbox: HeapCons<Sample>, outbox: HeapProd<Sample>) -> Self {
        Self {
            inbox,
            outbox,
            latest: 0.0,
            has_value: false,
        }
    }

    /// Consumes everything queued in ring A, keeping only the newest value.
    ///
    /// Returns how many events were absorbed.
    pub fn drain_inbox(&mut self) -> usize {
        let mut count = 0;
        while let Some((_, value)) = self.inbox.try_pop() {
            self.latest = value;
            self.has_value = true;
            count += 1;
        }
        count
    }

    /// Emits one point at `now`. Returns false if the display ring was full.
    pub fn emit(&mut self, now: f64) -> bool {
        if !self.has_value {
            return true;
        }
        self.outbox.try_push((now, self.latest)).is_ok()
    }

    /// One acquisition step: absorb input, emit one point.
    pub fn tick(&mut self, now: f64) -> bool {
        self.drain_inbox();
        self.emit(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringbuf::HeapRb;

    fn stage() -> (Acquisition, HeapProd<Sample>, HeapCons<Sample>) {
        let (in_prod, in_cons) = HeapRb::new(16).split();
        let (out_prod, out_cons) = HeapRb::new(16).split();
        (Acquisition::new(in_cons, out_prod), in_prod, out_cons)
    }

    #[test]
    fn emits_one_point_per_tick_regardless_of_input() {
        let (mut acq, mut input, mut output) = stage();
        input.try_push((0.0, 10.0)).unwrap();

        acq.tick(0.0);
        acq.tick(0.004);
        acq.tick(0.008);

        let points: Vec<_> = std::iter::from_fn(|| output.try_pop()).collect();
        assert_eq!(points.len(), 3, "one sample per tick");
        assert_eq!(
            points.iter().map(|(_, value)| *value).collect::<Vec<_>>(),
            vec![10.0, 10.0, 10.0],
            "the latest value holds until a new event arrives"
        );
    }

    #[test]
    fn keeps_only_the_newest_queued_value() {
        let (mut acq, mut input, mut output) = stage();
        input.try_push((0.0, 1.0)).unwrap();
        input.try_push((0.0, 2.0)).unwrap();
        input.try_push((0.0, 3.0)).unwrap();

        acq.tick(0.0);

        let points: Vec<_> = std::iter::from_fn(|| output.try_pop()).collect();
        assert_eq!(points, vec![(0.0, 3.0)]);
    }

    #[test]
    fn emits_nothing_before_the_first_event() {
        let (mut acq, _input, mut output) = stage();
        acq.tick(0.0);
        assert_eq!(output.try_pop(), None, "no value has ever arrived");
    }

    #[test]
    fn sample_timing_is_the_clock_not_the_event_time() {
        let (mut acq, mut input, mut output) = stage();
        input.try_push((0.0, 5.0)).unwrap();
        acq.tick(1.0);
        input.try_push((1.5, 9.0)).unwrap();
        acq.tick(1.004);

        let points: Vec<_> = std::iter::from_fn(|| output.try_pop()).collect();
        assert_eq!(points, vec![(1.0, 5.0), (1.004, 9.0)]);
    }

    #[test]
    fn a_full_display_ring_drops_instead_of_blocking() {
        let (mut input, in_cons) = HeapRb::new(4).split();
        let (out_prod, _out_cons) = HeapRb::new(2).split();
        let mut acq = Acquisition::new(in_cons, out_prod);
        input.try_push((0.0, 1.0)).unwrap();

        assert!(acq.tick(0.0));
        assert!(acq.tick(0.004));
        assert!(!acq.tick(0.008), "third push must report a full ring");
    }

    #[test]
    fn display_capacity_covers_the_window() {
        assert!(DISPLAY_CAPACITY >= SAMPLE_HZ as usize * WINDOW_SECS as usize);
    }
}
