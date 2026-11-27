use crate::core::{Input, OutputEvents, VoiceId, VoiceOutput};

use super::octave_settings::OctaveState;

use super::{Consumer, CONTROL_SLOTS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OctaveControl {
    Shift,
}

#[derive(Debug)]
pub struct Octave {
    voice: VoiceId,
    settings: OctaveState,
}

impl Octave {
    pub fn new(voice: VoiceId) -> Self {
        Self {
            voice,
            settings: OctaveState::INITIAL,
        }
    }

    fn shifted_note(&self, note: u8) -> u8 {
        (note as i16 + self.settings.shift as i16).clamp(0, 127) as u8
    }

    const CONTROL_MAP: [Option<OctaveControl>; CONTROL_SLOTS] = [
        Some(OctaveControl::Shift),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    ];
}

impl Consumer for Octave {
    async fn consume(&mut self, input: &Input, out: &mut OutputEvents) -> bool {
        let before = self.settings;
        let consumed = match *input {
            Input::NoteOn { voice, note, velocity } if voice == self.voice => {
                out.push(VoiceOutput::NoteOn {
                    voice,
                    note: self.shifted_note(note),
                    velocity,
                })
                .is_ok()
            }
            Input::NoteOff { voice, note, velocity } if voice == self.voice => {
                out.push(VoiceOutput::NoteOff {
                    voice,
                    note: self.shifted_note(note),
                    velocity,
                })
                .is_ok()
            }
            Input::ControlUpdate { voice, control, value, .. } if voice == self.voice => {
                if Self::CONTROL_MAP
                    .get(control as usize)
                    .copied()
                    .flatten()
                    .is_some()
                {
                    self.settings.shift = libm::roundf((value.clamp(0.0, 1.0) * 84.0) - 48.0) as i8;
                    true
                } else {
                    false
                }
            }
            _ => false,
        };
        self.settings != before || consumed
    }

    fn set_control_slot(&mut self, slot: u8, value: f32) {
        if let Some(OctaveControl::Shift) =
            Self::CONTROL_MAP.get(slot as usize).copied().flatten()
        {
            self.settings.shift = libm::roundf((value.clamp(0.0, 1.0) * 84.0) - 48.0) as i8;
        }
    }

    fn control_slot_value(&self, slot: u8) -> Option<f32> {
        matches!(
            Self::CONTROL_MAP.get(slot as usize).copied().flatten(),
            Some(OctaveControl::Shift)
        )
        .then_some((self.settings.shift as f32 + 48.0) / 84.0)
    }
}
