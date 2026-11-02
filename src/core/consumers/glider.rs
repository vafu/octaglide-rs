use log::info;
use midi_msg::{Channel, ChannelVoiceMsg, MidiMsg};

use crate::{
    anim::{
        Cmd, Settings,
        modulators::{Glide, Modulator},
    },
    app::Animator,
    core::{GlideState, Input, OutputEvents, VoiceId, control_to_u8, control_to_u32},
    notes,
};

#[derive(Debug)]
pub struct Glider {
    voice_id: VoiceId,
    output_channel: Channel,
    animator: Animator,
    settings: GlideState,
    ui_dirty: bool,
}

impl Glider {
    pub fn new(voice_id: VoiceId, output_channel: Channel, animator: Animator) -> Self {
        Self {
            ui_dirty: true,
            voice_id,
            output_channel,
            animator,
            settings: GlideState::INITIAL,
        }
    }

    async fn handle_note_on(&mut self, note: u8) -> bool {
        let Some(transition) = notes::edit_notes(self.voice_id, |notes| notes.press(note)).await
        else {
            return false;
        };
        if !self.settings.enabled {
            return false;
        }
        if let Some(from) = transition.previous_top {
            self.animator
                .send(Cmd::Start(Modulator::Glide(Glide::new(
                    self.voice_id,
                    self.output_channel,
                    self.settings,
                    from,
                    note,
                ))))
                .await
                .ok();
            true
        } else {
            false
        }
    }

    async fn handle_note_off(&mut self, note: u8) -> bool {
        let Some(release) = notes::edit_notes(self.voice_id, |notes| notes.release(note)).await
        else {
            return false;
        };
        if !self.settings.enabled {
            return false;
        }
        if let Some(release) = release
            && release.was_top
            && let Some(last_held) = release.new_top
        {
            self.animator
                .send(Cmd::Start(Modulator::Glide(Glide::new(
                    self.voice_id,
                    self.output_channel,
                    self.settings,
                    note,
                    last_held,
                ))))
                .await
                .ok();
            info!("sliding from {} back to {}", note, last_held);
            true
        } else {
            info!("canceling anim");
            self.animator.send(Cmd::Stop).await.ok();
            false
        }
    }

    fn apply_slider(&mut self, index: u8, value: u16) {
        match index {
            3 => self.settings.duration_ms = control_to_u32(value, 0, 4000),
            2 => self.settings.bend_range_semitones = control_to_u32(value, 1, 24) as f32,
            1 => self.settings.note_on_velocity = control_to_u8(value, 1, 127),
            0 => self.settings.enabled = value >= u16::MAX / 2,
            _ => {}
        }
    }
}

impl crate::ui::UiProvider for Glider {
    type Model = crate::ui::GliderUi;

    fn ui_snapshot(&self) -> Self::Model {
        crate::ui::GliderUi {
            enabled: self.settings.enabled,
            duration_ms: self.settings.duration_ms,
            bend_range_semitones: self.settings.bend_range_semitones,
            note_on_velocity: self.settings.note_on_velocity,
        }
    }

    fn has_updates(&mut self) -> bool {
        core::mem::take(&mut self.ui_dirty)
    }
}

impl super::Consumer for Glider {
    async fn consume(&mut self, input: &Input, _out: &mut OutputEvents) -> bool {
        let before = self.settings;
        let consumed = match input {
            Input::Midi(
                MidiMsg::ChannelVoice { msg, .. } | MidiMsg::RunningChannelVoice { msg, .. },
            ) => match msg {
                ChannelVoiceMsg::NoteOn { note, velocity } if *velocity > 0 => {
                    self.handle_note_on(*note).await
                }
                ChannelVoiceMsg::NoteOn { note, .. } | ChannelVoiceMsg::NoteOff { note, .. } => {
                    self.handle_note_off(*note).await
                }
                _ => false,
            },
            Input::Slider { index, value, .. } if *index < 4 => {
                self.apply_slider(*index, *value);
                true
            }
            Input::Encoder { increment } => {
                let step = if *increment { 1 } else { -1 };
                self.settings.duration_ms =
                    (self.settings.duration_ms as i32 + step * 10).clamp(0, 4000) as u32;
                true
            }
            Input::EncoderClick { pressed: false } => {
                self.settings.enabled = !self.settings.enabled;
                true
            }
            _ => false,
        };
        if self.settings != before {
            self.ui_dirty = true;
            self.animator
                .send(Cmd::Update(Settings::Glide(self.settings)))
                .await
                .ok();
        }
        consumed
    }
}
