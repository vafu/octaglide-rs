use heapless::Vec;
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
        consumers::CoreOutput,
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
    fn consume(&mut self, event: &super::super::MidiEvent) -> CoreOutput {
        let mut res = Vec::new();

        let Ok(ref midi_msg) = event.msg else {
            return res;
        };

        let MidiMsg::ChannelVoice { channel, msg } = midi_msg else {
            return res;
        };

        match msg {
            NoteOn { note, velocity } => {
                // Only process user NoteOn, not synthetic
                if !event.synthetic {
                    self.handle_note_on(*channel, *note, *velocity, &mut res);
                }
            }

            NoteOff { note, .. } => {
                if event.synthetic {
                    // Filter out synthetic NoteOff for held notes
                    if self.held_notes.contains(note) {
                        return res; // Filtered out
                    }
                    // Allow synthetic NoteOff for non-held notes (intermediate notes)
                    res.push(SendMidi(midi_msg.clone())).ok();
                } else {
                    // User NoteOff - always process
                    self.handle_note_off(*channel, *note, midi_msg, &mut res);
                }
            }

            _ => {}
        }
        res
    }
}



