use core::fmt::Debug;

use enum_dispatch::enum_dispatch;
use heapless::Vec;
use midi_msg::{Channel, ChannelVoiceMsg, MidiMsg};

pub type Messages = Option<Vec<MidiMsg, 3>>;

#[enum_dispatch]
pub trait Modulation {
    fn animate(&mut self, progress: f32, depth: f32, offset: f32) -> Messages;
    fn reset(&mut self) -> Messages;
}

#[derive(Debug)]
#[enum_dispatch(Modulation)]
pub enum Modulator {
    Glide,
}

const PITCHBEND_CENTER: f32 = 8192.0;
const PITCHBEND_MAX: u16 = PITCHBEND_CENTER as u16 * 2;
const DEFAULT_VELOCITY: u8 = 100;

const SYNTH_BEND_RANGE_SEMITONES: f32 = 2.0;

/// Glide modulator for smooth pitch transitions between notes.
///
/// # Contract
/// - Glide does NOT send NoteOn/NoteOff for the `from` or `to` notes passed as arguments.
/// - The caller (Glider) is responsible for managing the active state of user-held notes.
/// - Glide only manages intermediate notes triggered during the glide animation.
/// - The caller must send NoteOn for the destination note BEFORE starting the glide.
#[derive(Debug)]
pub struct Glide {
    ch: Channel,
    from: u8,
    to: u8,
    active_note: u8,
}
impl Glide {
    pub fn new(ch: Channel, from: u8, to: u8) -> Self {
        Self {
            ch,
            from,
            to,
            active_note: 0,
        }
    }

    fn calc_bend_msg(&self, semitones: f32) -> MidiMsg {
        let bend_fraction = (semitones / SYNTH_BEND_RANGE_SEMITONES).clamp(-1.0, 1.0);

        let bend_value = (PITCHBEND_CENTER + bend_fraction * PITCHBEND_CENTER) as u16;
        let bend = bend_value.clamp(0, PITCHBEND_MAX);

        MidiMsg::ChannelVoice {
            channel: self.ch,
            msg: ChannelVoiceMsg::PitchBend { bend },
        }
    }
}

impl Modulation for Glide {
    fn animate(&mut self, progress: f32, _depth: f32, _offset: f32) -> Messages {
        let mut messages = Vec::<MidiMsg, 3>::new();

        if self.from == self.to {
            return Some(messages);
        }

        let slide_range = self.to as f32 - self.from as f32;
        let inter_note = self.from as f32 + (slide_range * progress);

        let new_active_note: u8;
        let req_ct: f32;

        // Are we in range of the 'to' note? Prioritize it.
        let dest_ct = inter_note - self.to as f32;
        if dest_ct.abs() <= SYNTH_BEND_RANGE_SEMITONES {
            new_active_note = self.to;
            req_ct = dest_ct;
        }
        // If not, are we still in range of the *current* note? Stick to it.
        else {
            let active_ct = inter_note - self.active_note as f32;
            if active_ct.abs() <= SYNTH_BEND_RANGE_SEMITONES {
                new_active_note = self.active_note;
                req_ct = active_ct;
            }
            // We are out of range of both. Switch to a new intermediate note.
            else {
                new_active_note = libm::roundf(inter_note) as u8;
                req_ct = inter_note - new_active_note as f32;
            }
        }

        if new_active_note != self.active_note {
            // Only send NoteOn for intermediate notes, not the destination
            // (Glider already sent NoteOn for the destination before starting animation)
            if new_active_note != self.to {
                let _ = messages.push(MidiMsg::ChannelVoice {
                    channel: self.ch,
                    msg: ChannelVoiceMsg::NoteOn {
                        note: new_active_note,
                        velocity: DEFAULT_VELOCITY,
                    },
                });
            }

            // Only send NoteOff for intermediate notes, not user-held notes (from/to)
            // Glider controls when user-held notes are released
            if self.active_note != 0 && self.active_note != self.from && self.active_note != self.to {
                let _ = messages.push(MidiMsg::ChannelVoice {
                    channel: self.ch,
                    msg: ChannelVoiceMsg::NoteOff {
                        note: self.active_note,
                        velocity: 0,
                    },
                });
            }
            self.active_note = new_active_note;
        }

        let _ = messages.push(self.calc_bend_msg(req_ct));

        Some(messages)
    }

    fn reset(&mut self) -> Messages {
        self.active_note = self.from;

        let mut messages = Vec::<MidiMsg, 3>::new();
        // let _ = messages.push(MidiMsg::ChannelVoice {
        //     channel: self.ch,
        //     msg: ChannelVoiceMsg::NoteOn {
        //         note: self.from,
        //         velocity: DEFAULT_VELOCITY,
        //     },
        // });
        //
        // Also send an initial pitch bend message (which is 0.0, or center)
        let _ = messages.push(MidiMsg::ChannelVoice {
            channel: self.ch,
            msg: ChannelVoiceMsg::PitchBend {
                bend: PITCHBEND_CENTER as u16,
            },
        });

        Some(messages)
    }
}


