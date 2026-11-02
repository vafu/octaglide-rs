use midi_msg::{Channel, ChannelVoiceMsg, MidiMsg};

use crate::{
    anim::{
        Cmd, Settings,
        modulators::{Envelope, Modulator},
    },
    app::Animator,
    core::{EnvelopeState, Input, OutputEvents, VoiceId, control_to_u8},
};

#[derive(Debug)]
pub struct ModTrigger {
    voice_id: VoiceId,
    output_channel: Channel,
    output_cc: u8,
    settings: EnvelopeState,
    animator: Animator,
    ui_dirty: bool,
}

impl ModTrigger {
    pub fn new(
        voice_id: VoiceId,
        output_channel: Channel,
        output_cc: u8,
        animator: Animator,
    ) -> Self {
        Self {
            ui_dirty: true,
            voice_id,
            output_channel,
            output_cc,
            animator,
            settings: EnvelopeState::INITIAL,
        }
    }

    fn apply_slider(&mut self, index: u8, value: u16, alternate: bool) {
        let value = control_to_u8(value, 0, 127);
        match (alternate, index) {
            (false, 3) => self.settings.attack.duration = value,
            (false, 2) => self.settings.decay.duration = value,
            (_, 1) => self.settings.sustain = value,
            (false, 0) => self.settings.release.duration = value,
            (true, 3) => self.settings.attack.curve = value,
            (true, 2) => self.settings.decay.curve = value,
            (true, 0) => self.settings.release.curve = value,
            _ => {}
        }
    }
}

impl super::SliderTarget for ModTrigger {
    fn slider_value(&self, index: u8, alternate: bool) -> Option<u8> {
        match (alternate, index) {
            (false, 3) => Some(self.settings.attack.duration),
            (false, 2) => Some(self.settings.decay.duration),
            (_, 1) => Some(self.settings.sustain),
            (false, 0) => Some(self.settings.release.duration),
            // Curve edits deliberately bypass pickup; durations and sustain do not.
            _ => None,
        }
    }
}

impl crate::ui::UiProvider for ModTrigger {
    type Model = crate::ui::ModTriggerUi;

    fn ui_snapshot(&self) -> Self::Model {
        crate::ui::ModTriggerUi {
            name: "Envelope",
            output_cc: self.output_cc,
            attack: crate::ui::EnvelopeStageUi {
                duration: self.settings.attack.duration,
                curve: self.settings.attack.curve,
            },
            decay: crate::ui::EnvelopeStageUi {
                duration: self.settings.decay.duration,
                curve: self.settings.decay.curve,
            },
            sustain: self.settings.sustain,
            release: crate::ui::EnvelopeStageUi {
                duration: self.settings.release.duration,
                curve: self.settings.release.curve,
            },
            mode: self.settings.mode,
        }
    }

    fn has_updates(&mut self) -> bool {
        core::mem::take(&mut self.ui_dirty)
    }
}

impl super::Consumer for ModTrigger {
    async fn consume(&mut self, input: &Input, _out: &mut OutputEvents) -> bool {
        let before = self.settings;
        let consumed = match input {
            Input::Midi(
                MidiMsg::ChannelVoice {
                    msg: ChannelVoiceMsg::NoteOn { velocity, .. },
                    ..
                }
                | MidiMsg::RunningChannelVoice {
                    msg: ChannelVoiceMsg::NoteOn { velocity, .. },
                    ..
                },
            ) if *velocity > 0 => {
                self.animator
                    .send(Cmd::Start(Modulator::Envelope(Envelope::new(
                        self.voice_id,
                        self.output_channel,
                        self.output_cc,
                        self.settings,
                    ))))
                    .await
                    .ok();
                false
            }
            Input::Slider {
                index,
                value,
                alternate,
            } if *index < 4 => {
                self.apply_slider(*index, *value, *alternate);
                true
            }
            Input::Encoder { increment } => {
                let step = if *increment { 1 } else { -1 };
                self.settings.mode = (self.settings.mode as i32 + step).rem_euclid(3) as u8;
                true
            }
            _ => false,
        };
        if self.settings != before {
            self.ui_dirty = true;
            self.animator
                .send(Cmd::Update(Settings::Envelope(self.settings)))
                .await
                .ok();
        }
        consumed
    }
}
