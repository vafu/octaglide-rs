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
        modulators::{Glide, Modulator, glide},
    },
    app::{Animator, MidiSender},
    core::{MidiEvent, MidiOut},
    held_notes,
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
        held_notes::press(note);

        if let Some(from) = from_note {
            self.animator
                .send(Cmd::Start(Modulator::Glide(Glide::new(
                    channel, from, note, &glide::CONFIG,
                ))))
                .await
                .unwrap();
        } else {
            self.midi_sender
                .send(MidiOut {
                    msg: MidiMsg::ChannelVoice {
                        channel,
                        msg: ChannelVoiceMsg::NoteOn { note, velocity },
                    },
                    tag: "glider",
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
        held_notes::release(released_note);

        if let Some(&last_held) = self.held_notes.last()
            && was_active
        {
            self.animator
                .send(Cmd::Start(Modulator::Glide(Glide::new(
                    channel,
                    released_note,
                    last_held,
                    &glide::CONFIG,
                ))))
                .await
                .unwrap();
            info!("sliding from {} back to {}", released_note, last_held);
        } else {
            info!("canceling anim");
            self.midi_sender
                .send(MidiOut {
                    msg: midi_msg,
                    tag: "glider",
                })
                .await
                .unwrap();
            self.animator.send(Cmd::Stop).await.unwrap();
        }
    }
}

impl super::Consumer for Glider {
    async fn consume(&mut self, event: MidiEvent) -> Option<MidiEvent> {
        let MidiMsg::ChannelVoice { channel, msg } = event.msg else {
            return Some(event);
        };

        match msg {
            NoteOn { note, velocity } => {
                self.handle_note_on(channel, note, velocity).await;
                None
            }

            NoteOff { note, .. } => {
                self.handle_note_off(channel, note, event.msg).await;
                None
            }

            _ => Some(event),
        }
    }
}
