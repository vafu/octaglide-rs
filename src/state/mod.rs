//! Settings types shared by consumers and animations.

mod defaults;

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

    pub fn step(self, direction: i8) -> Self {
        let next = (self.index() as i32 + direction as i32).rem_euclid(Self::COUNT as i32) as u8;
        Self::from_index(next)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnvelopeState {
    pub attack: EnvelopeStageState,
    pub decay: EnvelopeStageState,
    pub release: EnvelopeStageState,
    pub sustain: u8,
    pub mode: EnvelopeMode,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlideState {
    pub enabled: bool,
    pub duration_ms: u32,
    pub bend_range_semitones: f32,
    pub note_on_velocity: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlideMode {
    Disabled,
    Enabled,
}

impl GlideMode {
    pub const COUNT: u8 = 2;

    pub const fn index(self) -> u8 {
        match self {
            Self::Disabled => 0,
            Self::Enabled => 1,
        }
    }

    pub const fn from_index(index: u8) -> Self {
        match index {
            1 => Self::Enabled,
            _ => Self::Disabled,
        }
    }

    pub fn step(self, direction: i8) -> Self {
        if direction == 0 {
            self
        } else {
            match self {
                Self::Disabled => Self::Enabled,
                Self::Enabled => Self::Disabled,
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OctaveState {
    pub shift: i8,
}
