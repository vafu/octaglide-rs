//! Settings types shared by consumers and animations.
//!
//! Instances belong to individual consumers. This module defines their data
//! shapes and defaults; it contains no global voice or routing state.

mod defaults;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Settings for one envelope ramp.
pub struct EnvelopeStageState {
    pub duration: u8,
    pub curve: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Envelope settings owned by a ModTrigger consumer.
pub struct EnvelopeState {
    pub attack: EnvelopeStageState,
    pub decay: EnvelopeStageState,
    pub release: EnvelopeStageState,
    pub sustain: u8,
    pub mode: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
/// Glide settings owned by a Glider consumer.
pub struct GlideState {
    pub enabled: bool,
    pub duration_ms: u32,
    pub bend_range_semitones: f32,
    pub note_on_velocity: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Transposition settings owned by an Octave consumer.
pub struct OctaveState {
    pub shift: i8,
}
