use crate::{
    core::{
        Animator, Cmd, EnvelopeMode, EnvelopeState, Input, Modulator, OutputEvents, Settings,
        VoiceId,
    },
};

use super::{Consumer, CONTROL_SLOTS};
pub mod calc;
mod envelope;
pub(crate) use self::envelope::Envelope;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModTriggerControl {
    AttackDuration,
    DecayDuration,
    Sustain,
    ReleaseDuration,
    AttackCurve,
    DecayCurve,
    ReleaseCurve,
}

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

    const CONTROL_MAP: [Option<ModTriggerControl>; CONTROL_SLOTS] = [
        Some(ModTriggerControl::ReleaseDuration),
        Some(ModTriggerControl::Sustain),
        Some(ModTriggerControl::DecayDuration),
        Some(ModTriggerControl::AttackDuration),
        Some(ModTriggerControl::ReleaseCurve),
        Some(ModTriggerControl::DecayCurve),
        Some(ModTriggerControl::AttackCurve),
        None,
    ];

    fn set(&mut self, control: ModTriggerControl, value: f32) {
        let value = normalized_to_u8(value);
        match control {
            ModTriggerControl::AttackDuration => self.settings.attack.duration = value,
            ModTriggerControl::DecayDuration => self.settings.decay.duration = value,
            ModTriggerControl::Sustain => self.settings.sustain = value,
            ModTriggerControl::ReleaseDuration => self.settings.release.duration = value,
            ModTriggerControl::AttackCurve => self.settings.attack.curve = value,
            ModTriggerControl::DecayCurve => self.settings.decay.curve = value,
            ModTriggerControl::ReleaseCurve => self.settings.release.curve = value,
        }
    }

    fn get(&self, control: ModTriggerControl) -> f32 {
        let value = match control {
            ModTriggerControl::AttackDuration => self.settings.attack.duration,
            ModTriggerControl::DecayDuration => self.settings.decay.duration,
            ModTriggerControl::Sustain => self.settings.sustain,
            ModTriggerControl::ReleaseDuration => self.settings.release.duration,
            ModTriggerControl::AttackCurve => self.settings.attack.curve,
            ModTriggerControl::DecayCurve => self.settings.decay.curve,
            ModTriggerControl::ReleaseCurve => self.settings.release.curve,
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
                if let Some(mapped) = Self::CONTROL_MAP.get(control as usize).copied().flatten() {
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
        if let Some(control) = Self::CONTROL_MAP.get(slot as usize).copied().flatten() {
            self.set(control, value);
        }
    }

    fn control_slot_value(&self, slot: u8) -> Option<f32> {
        Self::CONTROL_MAP
            .get(slot as usize)
            .copied()
            .flatten()
            .map(|control| self.get(control))
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
