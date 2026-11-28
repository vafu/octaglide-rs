//! Core's semantic voice, consumer, and control model.

mod clock;
pub mod consumers;
mod control;
mod core;
mod modulator;
mod notes;
mod voice;

use heapless::Vec;

use crate::output::OutputSender;

pub use self::clock::Clock;
pub use self::consumers::{
    Consumer, ConsumerKind, Routing, SlotConsumer, CONTROL_SLOTS,
};
pub use self::modulator::{Modulation, Trigger};
pub use self::consumers::envelope::settings::{
    EnvelopeMode, EnvelopeStageState, EnvelopeState,
};
pub use self::notes::{HeldNotes, NoteKind, NoteTransition};

#[cfg(target_os = "none")]
pub(crate) use control::normalize_adc_10bit;

/// A semantic output produced by a voice or modulator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VoiceOutput {
    NoteOn { voice: VoiceId, note: u8, velocity: u8 },
    NoteOff { voice: VoiceId, note: u8, velocity: u8 },
    /// -1.0..=1.0 of the configured bend range.
    PitchBend { voice: VoiceId, amount: f32 },
    /// 0.0..=1.0 control value for a voice output slot.
    Control { voice: VoiceId, slot: u8, value: f32 },
}

/// Total addressable consumers per voice.
pub const SLOT_COUNT: u8 = 16;

/// Owns voices and their consumers.
pub struct Core {
    voices: Vec<Voice, MAX_VOICES>,
    output: OutputSender,
}

/// Semantic input consumed by Core.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Input {
    Note(NoteTransition),
    ControlUpdate { voice: VoiceId, slot: u8, control: u8, value: f32 },
    /// Select one of a consumer's modes by index. The UI owns wrapping.
    ModeSet { voice: VoiceId, slot: u8, index: u8 },
}

/// Maximum semantic outputs passed between consumers in one step.
pub const OUTPUT_CAPACITY: usize = 8;
pub type OutputEvents = Vec<VoiceOutput, OUTPUT_CAPACITY>;

/// Physical voice: 16 addressable consumers plus its held-note stack.
///
/// Slots are flat and opaque to Core. Which consumer sits in which slot is a
/// default patch decided at construction; grouping them into sections is the
/// UI's business.
#[derive(Debug)]
pub struct Voice {
    pub(super) slots: [SlotConsumer; SLOT_COUNT as usize],
    pub(super) notes: HeldNotes,
}

pub const MAX_VOICES: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoiceId(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub voice: VoiceId,
    /// Flat slot index, `0..SLOT_COUNT`.
    pub slot: u8,
}

/// Snapshot of one consumer for UI display.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConsumerSnapshot {
    pub kind: ConsumerKind,
    pub output_slot: u8,
    pub controls: [Option<f32>; CONTROL_SLOTS],
    pub mode: u8,
    /// How many modes the consumer offers; zero means mode selection is moot.
    pub mode_count: u8,
}

#[cfg(test)]
mod test;
