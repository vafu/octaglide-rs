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
    Consumer, ConsumerKind, Routing, SlotConsumer, CONTROL_SLOTS, SLOTS_PER_SECTION,
};
pub use self::modulator::{Activation, Modulation};
pub use self::consumers::envelope::settings::{EnvelopeStageState, EnvelopeState};
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

/// One of the four addressable chains inside a voice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Sequencing,
    Voicing,
    Modulation,
    Reserved,
}

impl Section {
    pub const COUNT: u8 = 4;

    pub const ALL: [Section; 4] = [
        Section::Sequencing,
        Section::Voicing,
        Section::Modulation,
        Section::Reserved,
    ];

    pub const fn index(self) -> u8 {
        match self {
            Self::Sequencing => 0,
            Self::Voicing => 1,
            Self::Modulation => 2,
            Self::Reserved => 3,
        }
    }

    pub const fn from_index(index: u8) -> Self {
        match index {
            0 => Self::Sequencing,
            1 => Self::Voicing,
            2 => Self::Modulation,
            _ => Self::Reserved,
        }
    }

    pub const fn next(self) -> Self {
        Self::from_index((self.index() + 1) % Self::COUNT)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Sequencing => "SEQ",
            Self::Voicing => "VOI",
            Self::Modulation => "MOD",
            Self::Reserved => "RSV",
        }
    }
}

/// Address of one consumer: a section and a slot index within it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub section: Section,
    pub index: u8,
}

impl Slot {
    pub const fn new(section: Section, index: u8) -> Self {
        Self { section, index }
    }

    /// Flat index into a voice's `Section::COUNT * SLOTS_PER_SECTION` slots.
    pub const fn flat(self) -> usize {
        self.section.index() as usize * SLOTS_PER_SECTION + self.index as usize
    }

    pub const fn from_flat(flat: usize) -> Self {
        Self {
            section: Section::from_index((flat / SLOTS_PER_SECTION) as u8),
            index: (flat % SLOTS_PER_SECTION) as u8,
        }
    }
}

/// Owns voices and their consumers.
pub struct Core {
    voices: Vec<Voice, MAX_VOICES>,
    output: OutputSender,
}

/// Semantic input consumed by Core.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Input {
    Note(NoteTransition),
    ControlUpdate { voice: VoiceId, slot: Slot, control: u8, value: f32 },
    /// Select one of a consumer's modes by index. The UI owns wrapping.
    ModeSet { voice: VoiceId, slot: Slot, index: u8 },
}

/// Maximum semantic outputs passed between consumers in one step.
pub const OUTPUT_CAPACITY: usize = 8;
pub type OutputEvents = Vec<VoiceOutput, OUTPUT_CAPACITY>;

/// Physical voice: four sections of four addressable consumers.
#[derive(Debug)]
pub struct Voice {
    pub(super) slots: [[SlotConsumer; SLOTS_PER_SECTION]; Section::COUNT as usize],
    pub(super) notes: HeldNotes,
}

pub const MAX_VOICES: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoiceId(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub voice: VoiceId,
    pub slot: Slot,
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
