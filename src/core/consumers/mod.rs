//! Consumers: the addressable occupants of a voice's four sections.
//!
//! A consumer reacts to `Input`, exposes controls and modes to the UI, and may
//! also implement [`Modulation`] when it produces timed output.

use enum_dispatch::enum_dispatch;

use crate::core::{Input, OutputEvents};

pub mod envelope;
pub(crate) mod glide;
mod noop;
mod octave;

pub use self::envelope::ModTrigger;
pub use self::glide::Glider;
pub use self::noop::Noop;
pub use self::octave::Octave;

pub const CONTROL_SLOTS: usize = 8;

/// What a consumer decided to do with the input it received.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Routing {
    /// The event continues to the rest of the chain.
    Forward,
    /// The consumer handled the event; the chain stops here.
    Consumed,
}

/// Behaviour shared by every addressable consumer.
#[enum_dispatch]
pub trait Consumer {
    /// Handles one semantic input addressed to this consumer.
    fn consume(&mut self, input: &Input, out: &mut OutputEvents) -> Routing;

    /// Whether this consumer wants broadcast note events.
    ///
    /// Addressed events (`ControlUpdate`, `ModeSet`) always reach their slot;
    /// this only gates the note fan-out.
    fn wants_notes(&self) -> bool {
        false
    }

    fn control_slot_value(&self, _slot: u8) -> Option<f32> {
        None
    }

    /// Number of selectable modes; zero means mode selection is moot.
    fn mode_count(&self) -> u8 {
        0
    }

    /// Currently selected mode, as an index into `mode_count`.
    fn mode_index(&self) -> u8 {
        0
    }

    /// Selects a mode by index. Out-of-range indices are ignored.
    fn set_mode(&mut self, _index: u8) {}

    fn output_slot(&self) -> u8 {
        0
    }
}

/// The 16 consumers of one voice: four sections of four slots.
#[enum_dispatch(Consumer, Modulation)]
#[derive(Debug)]
pub enum SlotConsumer {
    Noop(Noop),
    Octave(Octave),
    Glider(Glider),
    ModTrigger(ModTrigger),
}

/// Stable identity of a consumer implementation, used in snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsumerKind {
    Noop,
    Octave,
    Glider,
    ModTrigger,
}
