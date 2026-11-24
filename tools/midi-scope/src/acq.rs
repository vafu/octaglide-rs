//! Acquisition stage: samples the latest MIDI value on a fixed clock.
//!
//! The scope is not driven by MIDI timing. The MIDI callback writes the newest
//! value into a shared atomic; this stage reads it and emits one point every
//! `SAMPLE_PERIOD` into the display ring. The UI drains that ring. A silent
//! input therefore still produces a flat, correctly-timed trace.
//!
//! Only the newest value matters between samples, so the MIDI-to-acquisition
//! hand-off is a single atomic rather than a queue.

use std::{sync::Arc, time::Duration};

use atomic_float::AtomicF32;
use ringbuf::{
    HeapProd,
    traits::Producer,
};

/// Samples per second emitted to the display ring.
pub const SAMPLE_HZ: u32 = 250;

/// Wall-clock spacing between emitted samples.
pub const SAMPLE_PERIOD: Duration = Duration::from_micros(1_000_000 / SAMPLE_HZ as u64);

/// Visible time span, in seconds.
pub const WINDOW_SECS: f64 = 10.0;

/// Display ring capacity: enough for the whole window at `SAMPLE_HZ`.
pub const DISPLAY_CAPACITY: usize = (SAMPLE_HZ as usize * WINDOW_SECS as usize) + 64;

/// One sampled value.
pub type Sample = f32;

/// The single value the MIDI callback publishes.
///
/// `NaN` means "no value yet"; any MIDI value is finite, so it doubles as the
/// has-value flag without a second atomic.
pub type Latest = Arc<AtomicF32>;

/// Creates the shared slot, empty until the first MIDI event.
pub fn latest_slot() -> Latest {
    Arc::new(AtomicF32::new(f32::NAN))
}

/// Publishes a value from the MIDI callback. Lock-free, never blocks.
pub fn publish(latest: &Latest, value: f32) {
    latest.store(value, std::sync::atomic::Ordering::Relaxed);
}

pub struct Acquisition {
    latest: Latest,
    outbox: HeapProd<Sample>,
}

impl Acquisition {
    pub fn new(latest: Latest, outbox: HeapProd<Sample>) -> Self {
        Self { latest, outbox }
    }

    /// Emits one value, holding the last one if nothing new arrived.
    ///
    /// Returns false if the display ring was full.
    pub fn tick(&mut self) -> bool {
        let value = self.latest.load(std::sync::atomic::Ordering::Relaxed);
        if value.is_nan() {
            return true;
        }
        self.outbox.try_push(value).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringbuf::{
        HeapCons, HeapRb,
        traits::{Consumer, Split},
    };

    fn stage() -> (Acquisition, Latest, HeapCons<Sample>) {
        let latest = latest_slot();
        let (out_prod, out_cons) = HeapRb::new(16).split();
        (Acquisition::new(latest.clone(), out_prod), latest, out_cons)
    }

    #[test]
    fn emits_one_point_per_tick_regardless_of_input() {
        let (mut acq, latest, mut output) = stage();
        publish(&latest, 10.0);

        acq.tick();
        acq.tick();
        acq.tick();

        let points: Vec<_> = std::iter::from_fn(|| output.try_pop()).collect();
        assert_eq!(points, vec![10.0, 10.0, 10.0], "the last value holds");
    }

    #[test]
    fn a_new_value_is_picked_up_on_the_next_tick() {
        let (mut acq, latest, mut output) = stage();
        publish(&latest, 1.0);
        acq.tick();
        publish(&latest, 2.0);
        acq.tick();

        let points: Vec<_> = std::iter::from_fn(|| output.try_pop()).collect();
        assert_eq!(points, vec![1.0, 2.0]);
    }

    #[test]
    fn only_the_newest_value_between_ticks_is_seen() {
        let (mut acq, latest, mut output) = stage();
        publish(&latest, 1.0);
        publish(&latest, 2.0);
        publish(&latest, 3.0);
        acq.tick();

        let points: Vec<_> = std::iter::from_fn(|| output.try_pop()).collect();
        assert_eq!(points, vec![3.0], "intermediate values are skipped");
    }

    #[test]
    fn emits_nothing_before_the_first_event() {
        let (mut acq, _latest, mut output) = stage();
        acq.tick();
        assert_eq!(output.try_pop(), None, "no value has ever arrived");
    }

    #[test]
    fn each_tick_takes_the_value_current_at_that_moment() {
        let (mut acq, latest, mut output) = stage();
        publish(&latest, 5.0);
        acq.tick();
        publish(&latest, 9.0);
        acq.tick();

        let points: Vec<_> = std::iter::from_fn(|| output.try_pop()).collect();
        assert_eq!(points, vec![5.0, 9.0]);
    }

    #[test]
    fn a_full_display_ring_drops_instead_of_blocking() {
        let latest = latest_slot();
        let (out_prod, _out_cons) = HeapRb::new(2).split();
        let mut acq = Acquisition::new(latest.clone(), out_prod);
        publish(&latest, 1.0);

        assert!(acq.tick());
        assert!(acq.tick());
        assert!(!acq.tick(), "third push must report a full ring");
    }

    #[test]
    fn display_capacity_covers_the_window() {
        assert!(DISPLAY_CAPACITY >= SAMPLE_HZ as usize * WINDOW_SECS as usize);
    }
}
