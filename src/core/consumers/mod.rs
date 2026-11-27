//! Consumers of semantic input, split by where they sit in a voice.

use enum_dispatch::enum_dispatch;

use crate::core::{Input, OutputEvents};

mod dispatch;
pub mod envelope;
pub(crate) mod glide;
mod octave;
mod passthrough;

pub use self::envelope::ModTrigger;
pub use self::glide::Glider;
pub use self::octave::Octave;
pub use self::passthrough::Passthrough;

pub const CONTROL_SLOTS: usize = 8;

/// Behaviour shared by every consumer, regardless of position.
#[enum_dispatch]
pub trait Consumer {
    async fn consume(&mut self, input: &Input, out: &mut OutputEvents) -> bool;
    fn set_control_slot(&mut self, _slot: u8, _value: f32) {}
    fn control_slot_value(&self, _slot: u8) -> Option<f32> {
        None
    }
}

/// A selectable modulation lane, addressed by the UI.
///
/// Only lanes carry modes and an output slot: they are the things a user can
/// select and edit. Voicing consumers have no addressable lane.
#[enum_dispatch]
pub trait ModLane: Consumer {
    /// Number of selectable modes; zero means the lane has none.
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

/// The voicing chain: ordered note transforms, applied before modulation lanes.
#[enum_dispatch(Consumer)]
#[derive(Debug)]
pub enum Voicing {
    Octave(Octave),
    Glider(Glider),
}

/// The four addressable modulation lanes.
#[enum_dispatch(ModLane)]
#[derive(Debug)]
pub enum ModLanes {
    Passthrough(Passthrough),
    ModTrigger(ModTrigger),
}

/// Stable consumer identity used in snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsumerKind {
    Passthrough,
    Octave,
    ModTrigger,
    Glider,
}

impl Consumer for ModLanes {
    async fn consume(&mut self, input: &Input, out: &mut OutputEvents) -> bool {
        match self {
            Self::Passthrough(inner) => inner.consume(input, out).await,
            Self::ModTrigger(inner) => inner.consume(input, out).await,
        }
    }

    fn set_control_slot(&mut self, slot: u8, value: f32) {
        match self {
            Self::Passthrough(inner) => inner.set_control_slot(slot, value),
            Self::ModTrigger(inner) => inner.set_control_slot(slot, value),
        }
    }

    fn control_slot_value(&self, slot: u8) -> Option<f32> {
        match self {
            Self::Passthrough(inner) => inner.control_slot_value(slot),
            Self::ModTrigger(inner) => inner.control_slot_value(slot),
        }
    }
}
