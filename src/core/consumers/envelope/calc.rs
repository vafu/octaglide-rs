//! Shared normalized math for playback and display.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Level {
    Zero,
    Full,
    Sustain,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StageKind {
    Ramp { from: Level, to: Level },
    Hold,
}

#[derive(Clone, Copy, Debug)]
pub enum StageSel {
    Attack,
    Decay,
    Release,
    None,
}

#[derive(Clone, Copy)]
pub struct StageDesc {
    pub kind: StageKind,
    pub sel: StageSel,
}

pub type Mode = &'static [StageDesc];

const AD_SEQ: Mode = &[
    StageDesc {
        kind: StageKind::Ramp {
            from: Level::Zero,
            to: Level::Full,
        },
        sel: StageSel::Attack,
    },
    StageDesc {
        kind: StageKind::Ramp {
            from: Level::Full,
            to: Level::Zero,
        },
        sel: StageSel::Decay,
    },
];

const AR_SEQ: Mode = &[
    StageDesc {
        kind: StageKind::Ramp {
            from: Level::Zero,
            to: Level::Full,
        },
        sel: StageSel::Attack,
    },
    StageDesc {
        kind: StageKind::Hold,
        sel: StageSel::None,
    },
    StageDesc {
        kind: StageKind::Ramp {
            from: Level::Full,
            to: Level::Zero,
        },
        sel: StageSel::Release,
    },
];

const ADSR_SEQ: Mode = &[
    StageDesc {
        kind: StageKind::Ramp {
            from: Level::Zero,
            to: Level::Full,
        },
        sel: StageSel::Attack,
    },
    StageDesc {
        kind: StageKind::Ramp {
            from: Level::Full,
            to: Level::Sustain,
        },
        sel: StageSel::Decay,
    },
    StageDesc {
        kind: StageKind::Hold,
        sel: StageSel::None,
    },
    StageDesc {
        kind: StageKind::Ramp {
            from: Level::Sustain,
            to: Level::Zero,
        },
        sel: StageSel::Release,
    },
];

pub fn sequence(mode: u8) -> Mode {
    match mode {
        1 => AR_SEQ,
        2 => ADSR_SEQ,
        _ => AD_SEQ,
    }
}

/// Interpolate a normalized ramp. Curve 64 is linear; higher values bend
/// above a rising ramp and below a falling ramp.
pub fn envelope_level(progress: f32, curve: u8, from: f32, to: f32) -> f32 {
    let offset = curve as f32 - 64.0;
    let signed_offset = if to < from { offset } else { -offset };
    let exponent = libm::powf(2.0, signed_offset / 32.0);
    let shaped = libm::powf(progress.clamp(0.0, 1.0), exponent);
    from + shaped * (to - from)
}

const HOLD_UNITS: f32 = 20.0;
const TIME_SPAN_UNITS: f32 = 3.0 * 127.0 + HOLD_UNITS;
const LAST_PIXEL: u32 = 127;
const PIXEL_UNITS: f32 = TIME_SPAN_UNITS / LAST_PIXEL as f32;

/// Samples the envelope preview at horizontal pixel `x`.
///
/// Reads the same `EnvelopeParams` the engine plays, so the plot cannot drift
/// from playback.
pub fn preview_level(envelope: &crate::core::EnvelopeParams, x: u32) -> f32 {
    let time = x.min(LAST_PIXEL) as f32 * TIME_SPAN_UNITS / LAST_PIXEL as f32;
    let sustain = envelope.sustain as f32 / 127.0;
    let attack = visible_duration(&envelope.attack);
    if attack > 0.0 && time <= attack {
        return ramp(time, &envelope.attack, 0.0, 1.0);
    }
    let time = time - attack;
    match envelope.mode.index() {
        0 => ramp(time, &envelope.decay, 1.0, 0.0),
        1 => {
            if time < HOLD_UNITS {
                1.0
            } else {
                ramp(time - HOLD_UNITS, &envelope.release, 1.0, 0.0)
            }
        }
        _ => {
            let decay = visible_duration(&envelope.decay);
            if time < decay {
                ramp(time, &envelope.decay, 1.0, sustain)
            } else if time < decay + HOLD_UNITS {
                sustain
            } else {
                ramp(time - decay - HOLD_UNITS, &envelope.release, sustain, 0.0)
            }
        }
    }
}

fn ramp(time: f32, stage: &crate::core::EnvelopeStageParams, from: f32, to: f32) -> f32 {
    let duration = visible_duration(stage);
    if time < duration {
        envelope_level(time / duration, stage.curve, from, to)
    } else {
        to
    }
}

fn visible_duration(stage: &crate::core::EnvelopeStageParams) -> f32 {
    if stage.duration == 0 {
        0.0
    } else {
        (stage.duration as f32).max(PIXEL_UNITS)
    }
}
