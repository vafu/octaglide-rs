use heapless::Vec;
use log::info;
use midi_msg::{
    Channel,
    ChannelVoiceMsg::{self, *},
    MidiMsg,
};

use crate::{
    anim::{
        animator::Cmd,
        modulators::{Glide, Modulator},
    },
    core::{
        MidiEvent,
        Output::*,
        consumers::{ConsumeResult, CoreOutput},
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

        if let Some(from) = from_note {
            // Start glide animation - it will handle the smooth transition
            res.push(Animate(Cmd::Start(Modulator::Glide(Glide::new(
                channel, from, note,
            )))))
            .ok();
        } else {
            // No previous note - send NoteOn for the first note
            res.push(SendMidi(MidiMsg::ChannelVoice {
                channel,
                msg: ChannelVoiceMsg::NoteOn { note, velocity },
            }))
            .ok();
        }
    }

    fn handle_note_off(
        &mut self,
        channel: Channel,
        note: u8,
        midi_msg: MidiMsg,
        res: &mut CoreOutput,
    ) {
        let Some(pos) = self.held_notes.iter().position(|&n| n == note) else {
            return;
        };

        let was_active = pos == self.held_notes.len() - 1;
        let released_note = self.held_notes.remove(pos);

        if let Some(&last_held) = self.held_notes.last()
            && was_active
        {
            // If we released the active note, slide back to the previous held note
            let _ = res.push(Animate(Cmd::Start(Modulator::Glide(Glide::new(
                channel,
                released_note,
                last_held,
            )))));
            info!("sliding from {} back to {}", released_note, last_held);
        } else {
            info!("canceling anim");
            // No more held notes - send the final NoteOff and stop animation
            res.push(SendMidi(midi_msg.clone())).unwrap();
            res.push(Animate(Cmd::Stop)).unwrap();
        }
    }
}

impl super::Consumer for Glider {
    fn consume(&mut self, event: MidiEvent) -> ConsumeResult {
        use super::ConsumeResult;

        let mut res = Vec::new();

        let MidiMsg::ChannelVoice { channel, msg } = event.msg else {
            return ConsumeResult::ignored(event);
        };

        // Synthetic messages: pass through with filtering
        if event.synthetic {
            match msg {
                NoteOff { note, .. } => {
                    // Filter out NoteOff for held notes, allow for intermediate notes
                    if !self.held_notes.contains(&note) {
                        res.push(SendMidi(event.msg)).ok();
                    } else {
                        info!("skip removing {}", &note);
                    }
                }
                _ => {
                    // Pass through all other synthetic messages (NoteOn, PitchBend, etc.)
                    res.push(SendMidi(event.msg)).ok();
                }
            }
            return ConsumeResult::Consumed(res);
        }

        // User messages: process normally
        match msg {
            NoteOn { note, velocity } => {
                self.handle_note_on(channel, note, velocity, &mut res);
                ConsumeResult::Consumed(res)
            }

            NoteOff { note, .. } => {
                self.handle_note_off(channel, note, event.msg, &mut res);
                ConsumeResult::Consumed(res)
            }

            // Ignore other messages (CC, PitchBend, etc.) - let passthrough handle them
            _ => ConsumeResult::ignored(event),
        }
    }
}
