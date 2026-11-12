use crate::core::{Input, OctaveState, OutputEvents, VoiceId, VoiceOutput};

use super::{Consumer, CONTROL_SLOTS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OctaveControl {
    Shift,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OctaveMode {
    Normal,
}

impl OctaveMode {
    pub const COUNT: u8 = 1;

    pub const fn index(self) -> u8 {
        0
    }
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

    pub(crate) fn set_control_slot(&mut self, slot: u8, value: f32) {
        if let Some(control) = Self::CONTROL_MAP.get(slot as usize).copied().flatten() {
            self.set_control(control, value);
        }
    }
}

impl Consumer for Octave {
    type Control = OctaveControl;
    type Mode = OctaveMode;

    const CONTROL_MAP: [Option<Self::Control>; CONTROL_SLOTS] = [
        Some(OctaveControl::Shift),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    ];
    const MODE_COUNT: u8 = 1;

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

    fn set_control(&mut self, control: Self::Control, value: f32) {
        if control == OctaveControl::Shift {
            self.settings.shift = libm::roundf((value.clamp(0.0, 1.0) * 84.0) - 48.0) as i8;
        }
    }

    fn control_value(&self, control: Self::Control) -> f32 {
        if control == OctaveControl::Shift {
            (self.settings.shift as f32 + 48.0) / 84.0
        } else {
            0.0
        }
    }

    fn mode(&self) -> Self::Mode {
        OctaveMode::Normal
    }

    fn update_mode(&mut self, _direction: i8) {}
}
