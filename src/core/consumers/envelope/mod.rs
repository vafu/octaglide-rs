use crate::core::{
    Consumer, Input, Modulation, NoteKind, NoteTransition, OutputEvents, Routing, Trigger, VoiceId,
};

use super::macros::consumer;
pub mod calc;
mod envelope;
pub(crate) use self::envelope::Envelope;
pub mod settings;
pub use self::settings::{EnvelopeMode, EnvelopeState};

/// A modulation lane that runs an envelope on note press.
#[derive(Debug)]
pub struct ModTrigger {
    voice_id: VoiceId,
    output_slot: u8,
    settings: EnvelopeState,
    engine: Option<Envelope>,
}

impl ModTrigger {
    pub fn new(voice_id: VoiceId, output_slot: u8) -> Self {
        Self {
            voice_id,
            output_slot,
            settings: EnvelopeState::INITIAL,
            engine: None,
        }
    }

    fn consume_input(&mut self, input: &Input, _out: &mut OutputEvents) -> Routing {
        match *input {
            Input::Note(NoteTransition { kind: NoteKind::Press, velocity, .. })
                if velocity > 0 =>
            {
                let mut engine = Envelope::new(self.voice_id, self.output_slot, self.settings);
                engine.trigger();
                self.engine = Some(engine);
                Routing::Forward
            }
            Input::ControlUpdate { control, value, .. } => {
                if Self::control_for_slot(control).is_some() {
                    self.set_control_slot(control, value);
                    Routing::Consumed
                } else {
                    Routing::Forward
                }
            }
            Input::ModeSet { index, .. } => {
                if index < EnvelopeMode::COUNT {
                    self.set_mode(index);
                    Routing::Consumed
                } else {
                    Routing::Forward
                }
            }
            _ => Routing::Forward,
        }
    }

    /// Keeps a running engine in sync with edited settings.
    fn after_control_set(&mut self) {
        if let Some(engine) = self.engine.as_mut() {
            engine.update(self.settings);
        }
    }
}

impl Modulation for ModTrigger {
    fn advance(&mut self, delta_ms: u32, gated: bool) -> Option<OutputEvents> {
        self.engine.as_mut()?.advance(delta_ms, gated)
    }

}

fn u8_from_normalized(value: f32) -> u8 {
    libm::roundf(value.clamp(0.0, 1.0) * 127.0) as u8
}

fn normalized_from_u8(value: u8) -> f32 {
    value as f32 / 127.0
}

/// Maps a normalized value onto the three trigger variants.
///
/// A discrete control spans the full 0..=1 range so each variant sits at a
/// round value (0.0, 0.5, 1.0) rather than being squeezed into 7 bits.
fn trigger_from_normalized(value: f32) -> Trigger {
    let steps = (Trigger::COUNT - 1) as f32;
    Trigger::from_index(libm::roundf(value.clamp(0.0, 1.0) * steps) as u8)
}

fn normalized_from_trigger(trigger: Trigger) -> f32 {
    trigger.index() as f32 / (Trigger::COUNT - 1) as f32
}

consumer! {
    ModTrigger {
        settings: EnvelopeState,
        wants_notes: true,
        output_slot: 0,
        controls: [
            0 => ReleaseDuration {
                set: |s: &mut EnvelopeState, v: f32| s.release.duration = u8_from_normalized(v),
                get: |s: &EnvelopeState| normalized_from_u8(s.release.duration),
            },
            1 => Sustain {
                set: |s: &mut EnvelopeState, v: f32| s.sustain = u8_from_normalized(v),
                get: |s: &EnvelopeState| normalized_from_u8(s.sustain),
            },
            2 => DecayDuration {
                set: |s: &mut EnvelopeState, v: f32| s.decay.duration = u8_from_normalized(v),
                get: |s: &EnvelopeState| normalized_from_u8(s.decay.duration),
            },
            3 => AttackDuration {
                set: |s: &mut EnvelopeState, v: f32| s.attack.duration = u8_from_normalized(v),
                get: |s: &EnvelopeState| normalized_from_u8(s.attack.duration),
            },
            4 => ReleaseCurve {
                set: |s: &mut EnvelopeState, v: f32| s.release.curve = u8_from_normalized(v),
                get: |s: &EnvelopeState| normalized_from_u8(s.release.curve),
            },
            5 => DecayCurve {
                set: |s: &mut EnvelopeState, v: f32| s.decay.curve = u8_from_normalized(v),
                get: |s: &EnvelopeState| normalized_from_u8(s.decay.curve),
            },
            6 => AttackCurve {
                set: |s: &mut EnvelopeState, v: f32| s.attack.curve = u8_from_normalized(v),
                get: |s: &EnvelopeState| normalized_from_u8(s.attack.curve),
            },
            7 => TriggerControl {
                set: |s: &mut EnvelopeState, v: f32| s.trigger = trigger_from_normalized(v),
                get: |s: &EnvelopeState| normalized_from_trigger(s.trigger),
            },
        ],
        modes: {
            count: EnvelopeMode::COUNT,
            get: |s: &EnvelopeState| s.mode.index(),
            set: |s: &mut EnvelopeState, index: u8| s.mode = EnvelopeMode::from_index(index),
        },
    }
}
