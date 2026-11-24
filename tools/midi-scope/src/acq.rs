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

use std::sync::atomic::AtomicU64;
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

/// One point in the scope: the value, plus anything the UI may want to draw
/// about how that point arrived.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DataPoint {
    pub value: f32,
    /// The producer received a value equal to the one already held, i.e. the
    /// source repeated itself. Consumed when sampled.
    pub is_duplicate: bool,
}

impl DataPoint {
    pub const EMPTY: Self = Self {
        value: f32::NAN,
        is_duplicate: false,
    };
}

/// One sampled value handed to the UI.
pub type Sample = DataPoint;

/// Shared slot the MIDI callback writes and the acquisition stage reads.
///
/// The value and the duplicate flag live in one atomic so a reader can never
/// observe a half-updated point. `NaN` means "no value yet".
pub type Latest = Arc<AtomicU64>;

fn encode(point: DataPoint) -> u64 {
    ((point.is_duplicate as u64) << 32) | point.value.to_bits() as u64
}

fn decode(raw: u64) -> DataPoint {
    DataPoint {
        value: f32::from_bits(raw as u32),
        is_duplicate: raw >> 32 != 0,
    }
}

/// Creates the shared slot, empty until the first MIDI event.
pub fn latest_slot() -> Latest {
    Arc::new(AtomicU64::new(encode(DataPoint::EMPTY)))
}

/// Publishes a point from the MIDI callback. Lock-free, never blocks.
pub fn publish(latest: &Latest, point: DataPoint) {
    latest.store(encode(point), std::sync::atomic::Ordering::Relaxed);
}

/// Reads the newest point, leaving the slot untouched.
pub fn peek(latest: &Latest) -> DataPoint {
    decode(latest.load(std::sync::atomic::Ordering::Relaxed))
}

/// Reads the newest point and clears its one-shot `is_duplicate` flag.
///
/// Clearing is what makes the flag a marker for a single sampling instant
/// rather than a permanent property of the value.
pub fn take(latest: &Latest) -> DataPoint {
    let previous = decode(
        latest
            .fetch_and(0xFFFF_FFFF, std::sync::atomic::Ordering::Relaxed),
    );
    DataPoint {
        value: previous.value,
        is_duplicate: previous.is_duplicate,
    }
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
        let point = take(&self.latest);
        if point.value.is_nan() {
            return true;
        }
        self.outbox.try_push(point).is_ok()
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
        publish(&latest, DataPoint { value: 10.0, is_duplicate: false });

        acq.tick();
        acq.tick();
        acq.tick();

        let points: Vec<_> = std::iter::from_fn(|| output.try_pop()).collect();
        assert_eq!(
            points.iter().map(|p| p.value).collect::<Vec<_>>(),
            vec![10.0, 10.0, 10.0],
            "the last value holds"
        );
    }

    #[test]
    fn a_new_value_is_picked_up_on_the_next_tick() {
        let (mut acq, latest, mut output) = stage();
        publish(&latest, DataPoint { value: 1.0, is_duplicate: false });
        acq.tick();
        publish(&latest, DataPoint { value: 2.0, is_duplicate: false });
        acq.tick();

        let points: Vec<_> = std::iter::from_fn(|| output.try_pop()).collect();
        assert_eq!(points.iter().map(|p| p.value).collect::<Vec<_>>(), vec![1.0, 2.0]);
    }

    #[test]
    fn only_the_newest_value_between_ticks_is_seen() {
        let (mut acq, latest, mut output) = stage();
        publish(&latest, DataPoint { value: 1.0, is_duplicate: false });
        publish(&latest, DataPoint { value: 2.0, is_duplicate: false });
        publish(&latest, DataPoint { value: 3.0, is_duplicate: false });
        acq.tick();

        let points: Vec<_> = std::iter::from_fn(|| output.try_pop()).collect();
        assert_eq!(
            points.iter().map(|p| p.value).collect::<Vec<_>>(),
            vec![3.0],
            "intermediate values are skipped"
        );
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
        publish(&latest, DataPoint { value: 5.0, is_duplicate: false });
        acq.tick();
        publish(&latest, DataPoint { value: 9.0, is_duplicate: false });
        acq.tick();

        let points: Vec<_> = std::iter::from_fn(|| output.try_pop()).collect();
        assert_eq!(points.iter().map(|p| p.value).collect::<Vec<_>>(), vec![5.0, 9.0]);
    }

    #[test]
    fn a_full_display_ring_drops_instead_of_blocking() {
        let latest = latest_slot();
        let (out_prod, _out_cons) = HeapRb::new(2).split();
        let mut acq = Acquisition::new(latest.clone(), out_prod);
        publish(&latest, DataPoint { value: 1.0, is_duplicate: false });

        assert!(acq.tick());
        assert!(acq.tick());
        assert!(!acq.tick(), "third push must report a full ring");
    }

    #[test]
    fn duplicate_flag_is_reported_once() {
        let latest = latest_slot();
        publish(
            &latest,
            DataPoint {
                value: 5.0,
                is_duplicate: true,
            },
        );
        assert!(take(&latest).is_duplicate, "first sample sees the flag");
        assert!(!take(&latest).is_duplicate, "flag was consumed");
    }

    #[test]
    fn consuming_the_flag_keeps_the_value() {
        let latest = latest_slot();
        publish(
            &latest,
            DataPoint {
                value: 42.0,
                is_duplicate: true,
            },
        );
        let _ = take(&latest);
        assert_eq!(take(&latest).value, 42.0, "value survives the clear");
    }

    #[test]
    fn empty_slot_is_not_a_duplicate() {
        let latest = latest_slot();
        let point = take(&latest);
        assert!(point.value.is_nan());
        assert!(!point.is_duplicate);
    }

    #[test]
    fn display_capacity_covers_the_window() {
        assert!(DISPLAY_CAPACITY >= SAMPLE_HZ as usize * WINDOW_SECS as usize);
    }
}
