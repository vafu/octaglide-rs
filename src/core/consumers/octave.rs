use crate::core::{
    Consumer, Input, Modulation, NoteKind, NoteTransition, OutputEvents, Routing, VoiceId,
    VoiceOutput,
};

use super::macros::consumer;

/// Shift in semitones, normalized across the full ±48 range.
const SHIFT_SPAN: f32 = 84.0;
const SHIFT_OFFSET: f32 = 48.0;

#[derive(Debug)]
pub struct Octave {
    voice: VoiceId,
    settings: OctaveSettings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OctaveSettings {
    pub shift: i8,
}

impl OctaveSettings {
    pub const INITIAL: Self = Self { shift: 0 };
}

impl Octave {
    pub fn new(voice: VoiceId) -> Self {
        Self {
            voice,
            settings: OctaveSettings::INITIAL,
        }
    }

    fn shifted_note(&self, note: u8) -> u8 {
        (note as i16 + self.settings.shift as i16).clamp(0, 127) as u8
    }

    fn consume_input(&mut self, input: &Input, out: &mut OutputEvents) -> Routing {
        match *input {
            Input::Note(NoteTransition { kind, note, velocity, .. }) => {
                let note = self.shifted_note(note);
                let output = match kind {
                    NoteKind::Press => VoiceOutput::NoteOn { voice: self.voice, note, velocity },
                    NoteKind::Release => VoiceOutput::NoteOff { voice: self.voice, note, velocity },
                };
                out.push(output).ok();
                Routing::Consumed
            }
            Input::ControlUpdate { control, value, .. } => {
                if Self::control_for_slot(control).is_some() {
                    self.set_control_slot(control, value);
                    Routing::Consumed
                } else {
                    Routing::Forward
                }
            }
            _ => Routing::Forward,
        }
    }

    fn after_control_set(&mut self) {}
}

consumer! {
    Octave {
        settings: OctaveSettings,
        wants_notes: true,
        output_slot: 0,
        controls: [
            0 => Shift {
                set: |settings: &mut OctaveSettings, value: f32| {
                    settings.shift =
                        libm::roundf((value.clamp(0.0, 1.0) * SHIFT_SPAN) - SHIFT_OFFSET) as i8;
                },
                get: |settings: &OctaveSettings| {
                    (settings.shift as f32 + SHIFT_OFFSET) / SHIFT_SPAN
                },
            },
        ],
    }
}

impl Modulation for Octave {}
