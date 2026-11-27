//! Core's semantic voice, consumer, and control model.

mod animator;
mod clock;
pub mod consumers;
mod control;
mod core;
mod modulator;
mod voice;

use heapless::Vec;

use crate::output::OutputSender;

pub use self::animator::{
    ANIMATORS_PER_VOICE, Animator, AnimationEngine, Cmd, Settings, VoiceAnimators,
};
pub use self::clock::Clock;
pub use self::consumers::{
    Consumer, ConsumerKind, Consumers, CONTROL_SLOTS,
};
pub use self::modulator::{Messages, Modulation, Modulator};
pub use crate::state::{
    EnvelopeMode, EnvelopeStageState, EnvelopeState, GlideMode, GlideState, OctaveState,
};

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

/// Owns voices and their consumer chains.
pub struct Core {
    voices: Vec<Voice, MAX_VOICES>,
    output: OutputSender,
}

/// Semantic input consumed by Core.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Input {
    NoteOn { voice: VoiceId, note: u8, velocity: u8 },
    NoteOff { voice: VoiceId, note: u8, velocity: u8 },
    ControlUpdate { voice: VoiceId, lane: usize, control: u8, value: f32 },
    /// Select one of a consumer's modes by index. The UI owns wrapping.
    ModeSet { voice: VoiceId, lane: usize, index: u8 },
}

/// Maximum semantic outputs passed between consumers in one step.
pub const OUTPUT_CAPACITY: usize = 8;
pub type OutputEvents = Vec<VoiceOutput, OUTPUT_CAPACITY>;

pub const MAX_VOICING_CONSUMERS: usize = 2;
pub const MODULATION_LANES: usize = 4;

/// Physical voice: voicing processors followed by exactly four modulation lanes.
#[derive(Debug)]
pub struct Voice {
    idx: usize,
    voicing: Vec<Consumers, MAX_VOICING_CONSUMERS>,
    modulations: [Consumers; MODULATION_LANES],
}

pub const MAX_VOICES: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoiceId(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub voice: VoiceId,
    pub lane: usize,
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
