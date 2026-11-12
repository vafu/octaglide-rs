use crate::calc::{Preview, Stage};

use super::{EnvelopeStageUi, ModTriggerUi};

/// Sample the preview on a fixed horizontal time scale, independent of settings.
/// Zero-duration stages are instantaneous; the line stays at zero after completion.
/// Positive stages shorter than one pixel get one pixel so brief attacks remain visible.
/// Hold modes use a fixed illustrative plateau; actual sustain follows note release.
pub(crate) fn envelope_plot_level(model: &ModTriggerUi, x: u32) -> f32 {
    crate::calc::preview_level(
        &Preview {
            mode: model.mode,
            attack: stage(&model.attack),
            decay: stage(&model.decay),
            release: stage(&model.release),
            sustain: model.sustain as f32 / 127.0,
        },
        x,
    )
}

fn stage(value: &EnvelopeStageUi) -> Stage {
    Stage {
        duration: value.duration,
        curve: value.curve,
    }
}
