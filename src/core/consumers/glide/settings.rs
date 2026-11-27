//! Glide settings owned by the glide consumer.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlideState {
    pub enabled: bool,
    pub duration_ms: u32,
    pub bend_range_semitones: f32,
    pub note_on_velocity: u8,
}

impl GlideState {
    pub const INITIAL: Self = Self {
        enabled: true,
        duration_ms: 200,
        bend_range_semitones: 2.0,
        note_on_velocity: 100,
    };
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
