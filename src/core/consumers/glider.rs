use heapless::Vec;
use log::info;
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

const MAX_HELD_NOTES: usize = 16;
pub struct Glider {
    held_notes: Vec<u8, MAX_HELD_NOTES>,
}
impl Glider {
    pub fn new() -> Self {
        Glider {
            held_notes: Vec::new(),
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
            // if pos == self.held_notes.len() {
            // if we have some held notes -- slide back to previous
            // let _ = res.push(Animate(Cmd::Start(Modulator::Glide(Glide::new(
            //     channel,
            //     released_note,
            //     old_note,
            // )))));
            res.push(SendMidi(MidiMsg::ChannelVoice {
                channel,
                msg: ChannelVoiceMsg::NoteOff {
                    note: released_note,
                    velocity: 100,
                },
            }))
            .ok();
            // }
        } else {
            // --- All Notes Off ---
            // This was the last note. Forward the NoteOff and stop the engine.
            let _ = res.push(SendMidi(midi_msg.clone()));
            let _ = res.push(Animate(Cmd::Stop));
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
                if let Some(pos) = self.held_notes.iter().position(|&n| n == *note) {
                    self.held_notes.remove(pos);
                }

                let from_note = self.held_notes.last().cloned();

                self.held_notes.push(*note).ok();

                // TODO: when animating, I need to forbid note off if we're gliding through the
                // note that is played as a glide side effect.
                //
                // if let Some(from) = from_note {
                //     res.push(Animate(Cmd::Start(Modulator::Glide(Glide::new(
                //         *channel, from, *note,
                //     )))))
                //     .ok();
                // } else {
                res.push(SendMidi(MidiMsg::ChannelVoice {
                    channel: *channel,
                    msg: ChannelVoiceMsg::NoteOn {
                        note: *note,
                        velocity: *velocity,
                    },
                }))
                .ok();
                // }
            }

            NoteOff { note, .. } => {
                self.handle_note_off(*channel, *note, midi_msg, &mut res);
            }

            _ => {}
        }
        res
    }
}
