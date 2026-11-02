//! Initial settings owned by individual consumers.

use super::{EnvelopeStageState, EnvelopeState, GlideState, OctaveState};

impl EnvelopeState {
    /// Initial settings for a newly configured consumer.
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
        mode: 0,
    };
}

impl GlideState {
    /// Initial settings for a newly configured consumer.
    pub const INITIAL: Self = Self {
        enabled: true,
        duration_ms: 200,
        bend_range_semitones: 2.0,
        note_on_velocity: 100,
    };
}

impl OctaveState {
    /// Initial settings for a newly configured consumer.
    pub const INITIAL: Self = Self { shift: 0 };
}
