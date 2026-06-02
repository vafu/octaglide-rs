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
    core::{MidiEvent, MidiOut},
    state,
};

const MAX_HELD_NOTES: usize = 8;
const MIDI_CHANNELS: usize = 16;

type HeldNoteStack = Vec<u8, MAX_HELD_NOTES>;

#[derive(Debug)]
pub struct Glider {
    held_by_channel: [HeldNoteStack; MIDI_CHANNELS],
    midi_sender: MidiSender,
    animator: Animator,
}

impl Glider {
    pub fn new(midi_sender: MidiSender, animator: Animator) -> Self {
        Glider {
            held_by_channel: core::array::from_fn(|_| Vec::new()),
            midi_sender,
            animator,
        }
    }

    async fn handle_note_on(&mut self, channel: Channel, note: u8, velocity: u8) {
        let from_note = {
            let held_notes = self.stack_mut(channel);
            if let Some(pos) = held_notes.iter().position(|&n| n == note) {
                held_notes.remove(pos);
            }
            let from_note = held_notes.last().cloned();
            held_notes.push(note).unwrap();
            from_note
        };
        state::edit_voice(channel, |voice| {
            voice.held_notes.press(note);
        })
        .await;

        if let Some(from) = from_note {
            self.animator
                .send(Cmd::Start(Modulator::Glide(Glide::new(
                    channel, from, note,
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
        let Some((was_active, released_note, last_held)) = ({
            let held_notes = self.stack_mut(channel);
            let Some(pos) = held_notes.iter().position(|&n| n == note) else {
                return;
            };

            let was_active = pos == held_notes.len() - 1;
            let released_note = held_notes.remove(pos);
            let last_held = held_notes.last().copied();
            Some((was_active, released_note, last_held))
        }) else {
            return;
        };

        state::edit_voice(channel, |voice| {
            voice.held_notes.release(released_note);
        })
        .await;

        if let Some(last_held) = last_held
            && was_active
        {
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

    fn stack_mut(&mut self, channel: Channel) -> &mut HeldNoteStack {
        &mut self.held_by_channel[channel as u8 as usize]
    }
}

impl super::Consumer for Glider {
    async fn consume(&mut self, event: MidiEvent) -> Option<MidiEvent> {
        let MidiMsg::ChannelVoice { channel, msg } = event.msg else {
            return Some(event);
        };

        if !state::has_voice(channel).await {
            return Some(event);
        }

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
