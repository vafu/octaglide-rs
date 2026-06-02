use crate::{
    core::event::{Event, EventPayload, EventRole, Events, control_to_i8},
    state::{self, ModifierParam, OctaveParam},
};

#[derive(Debug)]
pub struct Octave;

impl Octave {
    pub fn new() -> Self {
        Self
    }

    fn shifted_note(note: u8, shift: i8) -> u8 {
        (note as i16 + shift as i16).clamp(0, 127) as u8
    }
}

impl super::Consumer for Octave {
    async fn consume(&mut self, event: Event, out: &mut Events) {
        match event {
            Event {
                role:
                    EventRole::Modifier {
                        voice,
                        param: ModifierParam::Octave(OctaveParam::Shift),
                    },
                payload,
            } => {
                state::edit_voice(voice, |voice| match payload {
                    EventPayload::Control(value) => {
                        voice.octave.shift = control_to_i8(value, -48, 36);
                    }
                    EventPayload::Delta(delta) => {
                        voice.octave.shift = voice
                            .octave
                            .shift
                            .saturating_add((delta as i8).saturating_mul(12))
                            .clamp(-48, 36);
                    }
                    _ => {}
                })
                .await;
            }
            Event {
                role: EventRole::Voice(voice),
                payload: EventPayload::NoteOn { note, velocity },
            } => {
                let shift = state::read_voice(voice, |voice| voice.octave.shift)
                    .await
                    .unwrap_or(0);
                out.push(Event {
                    role: EventRole::Voice(voice),
                    payload: EventPayload::NoteOn {
                        note: Self::shifted_note(note, shift),
                        velocity,
                    },
                })
                .ok();
            }
            Event {
                role: EventRole::Voice(voice),
                payload: EventPayload::NoteOff { note, velocity },
            } => {
                let shift = state::read_voice(voice, |voice| voice.octave.shift)
                    .await
                    .unwrap_or(0);
                out.push(Event {
                    role: EventRole::Voice(voice),
                    payload: EventPayload::NoteOff {
                        note: Self::shifted_note(note, shift),
                        velocity,
                    },
                })
                .ok();
            }
            _ => {
                out.push(event).ok();
            }
        }
    }
}
