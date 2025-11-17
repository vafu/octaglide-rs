use heapless::Vec;
use log::debug;
use midi_msg::{
    Channel,
    ChannelVoiceMsg::{self, *},
    MidiMsg,
};

use crate::{
    anim::{
        engine::Cmd,
        modulators::{Glide, Modulator},
    },
    core::{
        Output::*,
        consumers::{Consumer, CoreOutput},
    },
};

const MAX_HELD_NOTES: usize = 8;
pub struct Glider {
    held_notes: Vec<u8, MAX_HELD_NOTES>,
}
impl Glider {
    pub fn new() -> Self {
        Glider {
            held_notes: Vec::new(),
        }
    }

    fn handle_note_on(&mut self, channel: Channel, note: u8, velocity: u8, res: &mut CoreOutput) {
        if let Some(pos) = self.held_notes.iter().position(|&n| n == note) {
            self.held_notes.remove(pos);
        }
        let from_note = self.held_notes.last().cloned();
        self.held_notes.push(note).unwrap();

        // Always send NoteOn for the new note first
        res.push(SendMidi(MidiMsg::ChannelVoice {
            channel,
            msg: ChannelVoiceMsg::NoteOn { note, velocity },
        }))
        .ok();

        // Then start glide animation if there was a previous note
        if let Some(from) = from_note {
            res.push(Animate(Cmd::Start(Modulator::Glide(Glide::new(
                channel, from, note,
            )))))
            .ok();
        }
    }

    fn handle_note_off(
        &mut self,
        channel: Channel,
        note: u8,
        midi_msg: &MidiMsg,
        res: &mut CoreOutput,
    ) {
        let Some(pos) = self.held_notes.iter().position(|&n| n == note) else {
            return;
        };
        let released_note = self.held_notes.remove(pos);
        if let Some(&old_note) = self.held_notes.last() {
            // Check if the note we released was the *active* one
            if pos == self.held_notes.len() {
                // if we have some held notes -- slide back to previous
                let _ = res.push(Animate(Cmd::Start(Modulator::Glide(Glide::new(
                    channel,
                    released_note,
                    old_note,
                )))));
                res.push(SendMidi(MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOff {
                        note: released_note,
                        velocity: 100,
                    },
                }))
                .ok();
            }
        } else {
            // --- All Notes Off ---
            // This was the last note. Forward the NoteOff and stop the engine.
            res.push(SendMidi(midi_msg.clone())).unwrap();
            res.push(Animate(Cmd::Stop)).unwrap();
        }
    }
}

impl super::Consumer for Glider {
    fn consume(&mut self, midi_msg: &MidiMsg) -> CoreOutput {
        let mut res = Vec::new();

        let MidiMsg::ChannelVoice { channel, msg } = midi_msg else {
            return res;
        };

        match msg {
            NoteOn { note, velocity } => {
                self.handle_note_on(*channel, *note, *velocity, &mut res);
            }

            NoteOff { note, .. } => {
                self.handle_note_off(*channel, *note, midi_msg, &mut res);
            }

            _ => {}
        }
        res
    }
}
