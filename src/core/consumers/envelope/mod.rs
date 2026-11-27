use crate::core::{
    Animator, Cmd, Input, Modulator, OutputEvents, Settings, VoiceId,
};
use crate::core::modulator::EnvelopeControl;

use super::{Consumer, ModLane};
pub mod calc;
mod envelope;
pub mod settings;
pub(crate) use self::envelope::Envelope;
pub use self::settings::{EnvelopeMode, EnvelopeStageState, EnvelopeState};

#[derive(Debug)]
pub struct ModTrigger {
    voice_id: VoiceId,
    output_slot: u8,
    settings: EnvelopeState,
    animator: Animator,
}

impl ModTrigger {
    pub const SLOTS: [u8; 7] = [0, 1, 2, 3, 4, 5, 6];

    pub fn new(voice_id: VoiceId, output_slot: u8, animator: Animator) -> Self {
        Self {
            voice_id,
            output_slot,
            settings: EnvelopeState::INITIAL,
            animator,
        }
    }

    pub fn mode(&self) -> EnvelopeMode {
        self.settings.mode
    }

    pub fn settings(&self) -> &EnvelopeState {
        &self.settings
    }

    pub fn output_slot(&self) -> u8 {
        self.output_slot
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
        }
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
        };
        value as f32 / 127.0
    }
}

impl Consumer for ModTrigger {
    async fn consume(&mut self, input: &Input, _out: &mut OutputEvents) -> bool {
        let before = self.settings;
        let consumed = match *input {
            Input::NoteOn { voice, velocity, .. } if voice == self.voice_id && velocity > 0 => {
                let _: Result<(), _> = self.animator
                    .send(Cmd::Start(Modulator::Envelope(Envelope::new(
                        self.voice_id,
                        self.output_slot,
                        self.settings,
                    ))))
                    .await;
                false
            }
            Input::ControlUpdate { voice, control, value, .. } if voice == self.voice_id => {
                if let Some(mapped) = Self::control_for_slot(control) {
                    self.set(mapped, value);
                    true
                } else {
                    false
                }
            }
            Input::ModeSet { voice, index, .. } if voice == self.voice_id => {
                if index < EnvelopeMode::COUNT {
                    self.settings.mode = EnvelopeMode::from_index(index);
                    true
                } else {
                    false
                }
            }
            _ => false,
        };
        if self.settings != before {
            let _: Result<(), _> = self.animator
                .send(Cmd::Update(Settings::Envelope(self.settings)))
                .await;
        }
        consumed
    }

    fn set_control_slot(&mut self, slot: u8, value: f32) {
        if let Some(control) = Self::control_for_slot(slot) {
            self.set(control, value);
        }
    }

    fn control_slot_value(&self, slot: u8) -> Option<f32> {
        Self::control_for_slot(slot).map(|control| self.get(control))
    }
}

impl ModLane for ModTrigger {
    fn mode_count(&self) -> u8 {
        EnvelopeMode::COUNT
    }

    fn mode_index(&self) -> u8 {
        self.settings.mode.index()
    }

    fn set_mode(&mut self, index: u8) {
        if index < EnvelopeMode::COUNT {
            self.settings.mode = EnvelopeMode::from_index(index);
        }
    }

    fn output_slot(&self) -> u8 {
        self.output_slot
    }
}

pub(crate) fn normalized_to_u8(value: f32) -> u8 {
    libm::roundf(value.clamp(0.0, 1.0) * 127.0) as u8
}

pub(crate) fn normalized_to_u32(value: f32, min: u32, max: u32) -> u32 {
    let span = max.saturating_sub(min);
    min + libm::roundf(value.clamp(0.0, 1.0) * span as f32) as u32
}
