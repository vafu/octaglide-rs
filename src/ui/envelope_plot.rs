use crate::anim::modulators::envelope_level;

use super::{EnvelopeStageUi, ModTriggerUi};

// All modes use the same scale: room for three maximum-length ramps and a
// schematic sustain plateau. Each duration unit corresponds to 16 ms in playback.
const HOLD_UNITS: f32 = 20.0;
const TIME_SPAN_UNITS: f32 = 3.0 * 127.0 + HOLD_UNITS;
const LAST_PIXEL: u32 = 127;
const PIXEL_UNITS: f32 = TIME_SPAN_UNITS / LAST_PIXEL as f32;

/// Sample the preview on a fixed horizontal time scale, independent of settings.
/// Zero-duration stages are instantaneous; the line stays at zero after completion.
/// Positive stages shorter than one pixel get one pixel so brief attacks remain visible.
/// Hold modes use a fixed illustrative plateau; actual sustain follows note release.
pub(crate) fn envelope_plot_level(model: &ModTriggerUi, x: u32) -> f32 {
    let time = x.min(LAST_PIXEL) as f32 * TIME_SPAN_UNITS / LAST_PIXEL as f32;
    let attack = visible_duration(&model.attack);
    if attack > 0.0 && time <= attack {
        return ramp(time, &model.attack, 0.0, 1.0);
    }
    let time = time - attack;
    match model.mode {
        0 => ramp(time, &model.decay, 1.0, 0.0),
        1 => {
            if time < HOLD_UNITS {
                1.0
            } else {
                ramp(time - HOLD_UNITS, &model.release, 1.0, 0.0)
            }
        }
        _ => {
            let decay = visible_duration(&model.decay);
            let sustain = model.sustain as f32 / 127.0;
            if time < decay {
                ramp(time, &model.decay, 1.0, sustain)
            } else if time < decay + HOLD_UNITS {
                sustain
            } else {
                ramp(time - decay - HOLD_UNITS, &model.release, sustain, 0.0)
            }
        }
    }
}

fn ramp(time: f32, stage: &EnvelopeStageUi, from: f32, to: f32) -> f32 {
    let duration = visible_duration(stage);
    if time < duration {
        envelope_level(time / duration, stage.curve, from, to)
    } else {
        to
    }
}

fn visible_duration(stage: &EnvelopeStageUi) -> f32 {
    if stage.duration == 0 {
        0.0
    } else {
        (stage.duration as f32).max(PIXEL_UNITS)
    }
}
