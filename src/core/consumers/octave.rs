use midi_msg::{ChannelVoiceMsg, MidiMsg};

use crate::core::{Input, OctaveState, OutputEvents, control_to_i8};

#[derive(Debug)]
pub struct Octave {
    settings: OctaveState,
    ui_dirty: bool,
}

impl Octave {
    pub fn new() -> Self {
        Self {
            ui_dirty: true,
            settings: OctaveState::INITIAL,
        }
    }

    fn shifted_note(&self, note: u8) -> u8 {
        (note as i16 + self.settings.shift as i16).clamp(0, 127) as u8
    }
}

impl crate::ui::UiProvider for Octave {
    type Model = crate::ui::OctaveUi;

    fn ui_snapshot(&self) -> Self::Model {
        crate::ui::OctaveUi {
            shift: self.settings.shift,
        }
    }

    fn has_updates(&mut self) -> bool {
        core::mem::take(&mut self.ui_dirty)
    }
}

impl super::Consumer for Octave {
    async fn consume(&mut self, input: &Input, out: &mut OutputEvents) -> bool {
        let before = self.settings;
        let consumed = match input {
            Input::Midi(original) => {
                let (MidiMsg::ChannelVoice { msg, .. } | MidiMsg::RunningChannelVoice { msg, .. }) =
                    original
                else {
                    return false;
                };
                let (ChannelVoiceMsg::NoteOn { note, .. } | ChannelVoiceMsg::NoteOff { note, .. }) =
                    msg
                else {
                    return false;
                };
                let shifted = self.shifted_note(*note);
                if shifted == *note {
                    return false;
                }
                let mut replacement = original.clone();
                if let MidiMsg::ChannelVoice { msg, .. }
                | MidiMsg::RunningChannelVoice { msg, .. } = &mut replacement
                {
                    match msg {
                        ChannelVoiceMsg::NoteOn { note, .. }
                        | ChannelVoiceMsg::NoteOff { note, .. } => *note = shifted,
                        _ => unreachable!(),
                    }
                }
                out.push(replacement).is_ok()
            }
            Input::Slider {
                index: 3, value, ..
            } => {
                self.settings.shift = control_to_i8(*value, -48, 36);
                true
            }
            Input::Encoder { increment } => {
                let step = if *increment { 1 } else { -1 };
                self.settings.shift = (self.settings.shift as i32 + step * 12).clamp(-48, 36) as i8;
                true
            }
            _ => false,
        };
        self.ui_dirty |= self.settings != before;
        consumed
    }
}
