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
    app::{Animator, MidiSender},
    core::MidiEvent,
};

const MAX_HELD_NOTES: usize = 8;

#[derive(Debug)]
pub struct Glider {
    held_notes: Vec<u8, MAX_HELD_NOTES>,
    midi_sender: MidiSender,
    animator: Animator,
}

impl Glider {
    pub fn new(midi_sender: MidiSender, animator: Animator) -> Self {
        Glider {
            held_notes: Vec::new(),
            midi_sender,
            animator,
        }
    }

    async fn handle_note_on(&mut self, channel: Channel, note: u8, velocity: u8) {
        if let Some(pos) = self.held_notes.iter().position(|&n| n == note) {
            self.held_notes.remove(pos);
        }
        let from_note = self.held_notes.last().cloned();
        self.held_notes.push(note).unwrap();

        if let Some(from) = from_note {
            // Start glide animation - it will handle the smooth transition
            self.animator
                .send(Cmd::Start(Modulator::Glide(Glide::new(channel, from, note))))
                .await
                .unwrap();
        } else {
            // No previous note - send NoteOn for the first note
            self.midi_sender
                .send(MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOn { note, velocity },
                })
                .await
                .unwrap();
        }
    }

    async fn handle_note_off(&mut self, channel: Channel, note: u8, midi_msg: MidiMsg) {
        let Some(pos) = self.held_notes.iter().position(|&n| n == note) else {
            return;
        };

        let was_active = pos == self.held_notes.len() - 1;
        let released_note = self.held_notes.remove(pos);

        if let Some(&last_held) = self.held_notes.last()
            && was_active
        {
            // If we released the active note, slide back to the previous held note
            self.animator
                .send(Cmd::Start(Modulator::Glide(Glide::new(
                    channel,
                    released_note,
                    last_held,
                ))))
                .await
                .unwrap();
            info!("sliding from {} back to {}", released_note, last_held);
        } else {
            info!("canceling anim");
            // No more held notes - send the final NoteOff and stop animation
            self.midi_sender.send(midi_msg).await.unwrap();
            self.animator.send(Cmd::Stop).await.unwrap();
        }
    }
}

impl super::Consumer for Glider {
    async fn consume(&mut self, event: MidiEvent) -> Option<MidiEvent> {
        let MidiMsg::ChannelVoice { channel, msg } = event.msg else {
            return Some(event);
        };

        // Synthetic messages: pass through with filtering
        if event.synthetic {
            match msg {
                NoteOff { note, .. } => {
                    // Filter out NoteOff for held notes, allow for intermediate notes
                    if !self.held_notes.contains(&note) {
                        self.midi_sender.send(event.msg).await.unwrap();
                    } else {
                        info!("skip removing {}", &note);
                    }
                }
                _ => {
                    // Pass through all other synthetic messages (NoteOn, PitchBend, etc.)
                    self.midi_sender.send(event.msg).await.unwrap();
                }
            }
            return None;
        }

        // User messages: process normally
        match msg {
            NoteOn { note, velocity } => {
                self.handle_note_on(channel, note, velocity).await;
                None
            }

            NoteOff { note, .. } => {
                self.handle_note_off(channel, note, event.msg).await;
                None
            }

            // Ignore other messages (CC, PitchBend, etc.) - let passthrough handle them
            _ => Some(event),
        }
    }
}
