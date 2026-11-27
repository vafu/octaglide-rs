use crate::core::{
    Activation, Input, Modulation, NoteKind, NoteTransition, OutputEvents, Routing, VoiceId,
};
use crate::core::consumers::registry::EnvelopeControl;

use super::Consumer;
pub(crate) use self::envelope::Envelope;
pub mod calc;
mod envelope;
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

    fn set(&mut self, control: EnvelopeControl, value: f32) {
        let value = normalized_to_u8(value);
        match control {
            EnvelopeControl::AttackDuration => self.settings.attack.duration = value,
            EnvelopeControl::DecayDuration => self.settings.decay.duration = value,
            EnvelopeControl::Sustain => self.settings.sustain = value,
            EnvelopeControl::ReleaseDuration => self.settings.release.duration = value,
            EnvelopeControl::AttackCurve => self.settings.attack.curve = value,
            EnvelopeControl::DecayCurve => self.settings.decay.curve = value,
            EnvelopeControl::ReleaseCurve => self.settings.release.curve = value,
            EnvelopeControl::Activation => {
                self.settings.activation = match value {
                    0 => Activation::NoteOnce,
                    1 => Activation::NoteLoop,
                    _ => Activation::FreeLoop,
                }
            }
        }
        self.push_settings();
    }

    fn get(&self, control: EnvelopeControl) -> f32 {
        let value = match control {
            EnvelopeControl::AttackDuration => self.settings.attack.duration,
            EnvelopeControl::DecayDuration => self.settings.decay.duration,
            EnvelopeControl::Sustain => self.settings.sustain,
            EnvelopeControl::ReleaseDuration => self.settings.release.duration,
            EnvelopeControl::AttackCurve => self.settings.attack.curve,
            EnvelopeControl::DecayCurve => self.settings.decay.curve,
            EnvelopeControl::ReleaseCurve => self.settings.release.curve,
            EnvelopeControl::Activation => match self.settings.activation {
                Activation::NoteOnce => 0,
                Activation::NoteLoop => 1,
                Activation::FreeLoop => 2,
            },
        };
        value as f32 / 127.0
    }

    fn push_settings(&mut self) {
        if let Some(engine) = self.engine.as_mut() {
            engine.update(self.settings);
        }
    }
}

impl Consumer for ModTrigger {
    fn consume(&mut self, input: &Input, _out: &mut OutputEvents) -> Routing {
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

    fn wants_notes(&self) -> bool {
        true
    }

    fn set_control_slot(&mut self, slot: u8, value: f32) {
        if let Some(control) = Self::control_for_slot(slot) {
            self.set(control, value);
        }
    }

    fn control_slot_value(&self, slot: u8) -> Option<f32> {
        Self::control_for_slot(slot).map(|control| self.get(control))
    }

    fn mode_count(&self) -> u8 {
        EnvelopeMode::COUNT
    }

    fn mode_index(&self) -> u8 {
        self.settings.mode.index()
    }

    fn set_mode(&mut self, index: u8) {
        if index < EnvelopeMode::COUNT {
            self.settings.mode = EnvelopeMode::from_index(index);
            self.push_settings();
        }
    }

    fn output_slot(&self) -> u8 {
        self.output_slot
    }
}

impl Modulation for ModTrigger {
    fn advance(&mut self, delta_ms: u32, gated: bool) -> Option<OutputEvents> {
        self.engine.as_mut()?.advance(delta_ms, gated)
    }

    fn reset(&mut self) -> OutputEvents {
        self.engine.as_mut().map(Modulation::reset).unwrap_or_default()
    }
}

pub(crate) fn normalized_to_u8(value: f32) -> u8 {
    libm::roundf(value.clamp(0.0, 1.0) * 127.0) as u8
}
