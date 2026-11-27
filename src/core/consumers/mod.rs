use enum_dispatch::enum_dispatch;

use crate::core::{Input, OutputEvents};

mod dispatch;
pub mod envelope;
pub(crate) mod glide;
mod octave;

pub use self::envelope::ModTrigger;
pub use self::glide::Glider;
pub use self::octave::Octave;
pub use self::passthrough::Passthrough;

pub const CONTROL_SLOTS: usize = 8;

mod passthrough;

/// Processes semantic input independently of MIDI, hardware, or display.
#[enum_dispatch]
pub trait Consumer {
    async fn consume(&mut self, input: &Input, out: &mut OutputEvents) -> bool;
    fn set_control_slot(&mut self, _slot: u8, _value: f32) {}
    fn control_slot_value(&self, _slot: u8) -> Option<f32> {
        None
    }
    /// Number of selectable modes; zero means the consumer has none.
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

/// Consumer instances.
#[enum_dispatch(Consumer)]
#[derive(Debug)]
pub enum Consumers {
    Passthrough(Passthrough),
    Octave(Octave),
    ModTrigger(ModTrigger),
    Glider(Glider),
}

/// Stable consumer identity used in snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsumerKind {
    Passthrough,
    Octave,
    ModTrigger,
    Glider,
}
