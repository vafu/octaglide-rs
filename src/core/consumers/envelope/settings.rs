//! Envelope settings owned by the envelope consumer.

use crate::core::Activation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnvelopeStageState {
    pub duration: u8,
    pub curve: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvelopeMode {
    Ad,
    Ar,
    Adsr,
}

impl EnvelopeMode {
    pub const COUNT: u8 = 3;

    pub const fn index(self) -> u8 {
        match self {
            Self::Ad => 0,
            Self::Ar => 1,
            Self::Adsr => 2,
        }
    }

    pub const fn from_index(index: u8) -> Self {
        match index {
            1 => Self::Ar,
            2 => Self::Adsr,
            _ => Self::Ad,
        }
    }

}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnvelopeState {
    pub attack: EnvelopeStageState,
    pub decay: EnvelopeStageState,
    pub release: EnvelopeStageState,
    pub sustain: u8,
    pub mode: EnvelopeMode,
    pub activation: Activation,
}

impl EnvelopeState {
    pub const INITIAL: Self = Self {
        attack: EnvelopeStageState {
            duration: 20,
            curve: 64,
        },
        decay: EnvelopeStageState {
            duration: 20,
            curve: 64,
        },
        release: EnvelopeStageState {
            duration: 20,
            curve: 64,
        },
        sustain: 80,
        mode: EnvelopeMode::Ad,
        activation: Activation::NoteOnce,
    };
}
