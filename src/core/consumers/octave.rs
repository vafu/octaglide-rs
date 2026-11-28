//! Octave: a voicing consumer that transposes notes.

use crate::core::{
    Consumer, Input, Modulation, NoteKind, NoteTransition, OutputEvents, Routing, VoiceId,
    VoiceOutput,
};

/// Shift in semitones, normalized across the full ±48 range.
const SHIFT_SPAN: f32 = 84.0;
const SHIFT_OFFSET: f32 = 48.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OctaveControl {
    Shift,
}

impl OctaveControl {
    const MAP: [Option<OctaveControl>; crate::core::CONTROL_SLOTS] = [
        Some(OctaveControl::Shift),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    ];

    fn for_slot(slot: u8) -> Option<Self> {
        Self::MAP.get(slot as usize).copied().flatten()
    }
}

#[derive(Debug)]
pub struct Octave {
    voice: VoiceId,
    shift: i8,
}

impl Octave {
    pub fn new(voice: VoiceId) -> Self {
        Self { voice, shift: 0 }
    }

    fn shifted_note(&self, note: u8) -> u8 {
        (note as i16 + self.shift as i16).clamp(0, 127) as u8
    }

    fn set(&mut self, control: OctaveControl, value: f32) {
        match control {
            OctaveControl::Shift => {
                self.shift = libm::roundf((value.clamp(0.0, 1.0) * SHIFT_SPAN) - SHIFT_OFFSET) as i8
            }
        }
    }

    fn get(&self, control: OctaveControl) -> f32 {
        match control {
            OctaveControl::Shift => (self.shift as f32 + SHIFT_OFFSET) / SHIFT_SPAN,
        }
    }
    /// Applies a control by slot. Out-of-range or padding slots are ignored.
    pub fn set_control(&mut self, slot: u8, value: f32) {
        if let Some(control) = OctaveControl::for_slot(slot) {
            self.set(control, value);
        }
    }

}

impl Consumer for Octave {
    fn consume(&mut self, input: &Input, out: &mut OutputEvents) -> Routing {
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
                if OctaveControl::for_slot(control).is_some() {
                    self.set_control(control, value);
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


    fn control_slot_value(&self, slot: u8) -> Option<f32> {
        OctaveControl::for_slot(slot).map(|control| self.get(control))
    }
}

impl Modulation for Octave {}
