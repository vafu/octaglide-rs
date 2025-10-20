use core::fmt::Debug;

use enum_dispatch::enum_dispatch;
use heapless::Vec;
use log::info;
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
const PITCHBEND_MAX: f32 = 16383.0;
const PITCHBEND_MIN: f32 = 0.0;
const DEFAULT_VELOCITY: u8 = 100;

const SYNTH_BEND_RANGE_SEMITONES: f32 = 2.0;

const POSITIVE_BEND_RADIUS: f32 = PITCHBEND_MAX - PITCHBEND_CENTER; // 8191
const NEGATIVE_BEND_RADIUS: f32 = PITCHBEND_CENTER - PITCHBEND_MIN; // 8192
//
#[derive(Debug)]
pub struct Glide {
    ch: Channel,
    from: u8,
    to: u8,
    prev_base: u8,
}
impl Glide {
    pub fn new(ch: Channel, from: u8, to: u8) -> Self {
        Glide {
            ch,
            from,
            to,
            prev_base: 0,
        }
    }
}

impl Modulation for Glide {
    fn animate(&mut self, progress: f32, _depth: f32, _offset: f32) -> Messages {
        let mut messages = Vec::<MidiMsg, 3>::new();

        if self.from == self.to || SYNTH_BEND_RANGE_SEMITONES == 0.0 {
            return None;
        }

        let slide_range = self.to as f32 - self.from as f32;
        let fbase_note = self.from as f32 + (slide_range * progress);
        let base_note = libm::roundf(fbase_note) as u8;

        if base_note != self.prev_base {
            let _ = messages.push(MidiMsg::ChannelVoice {
                channel: self.ch,
                msg: ChannelVoiceMsg::NoteOn {
                    note: base_note,
                    velocity: DEFAULT_VELOCITY,
                },
            });
            if self.prev_base != 0 {
                let _ = messages.push(MidiMsg::ChannelVoice {
                    channel: self.ch,
                    msg: ChannelVoiceMsg::NoteOff {
                        note: self.prev_base,
                        velocity: 0,
                    },
                });
            }
            self.prev_base = base_note;
        }

        let bend_semi = fbase_note - base_note as f32;
        let fbend = (bend_semi / SYNTH_BEND_RANGE_SEMITONES).clamp(-0.5, 0.5);

        let bend_offset = if fbend > 0.0 {
            fbend * 2.0 * POSITIVE_BEND_RADIUS
        } else {
            fbend * 2.0 * NEGATIVE_BEND_RADIUS
        };
        let bend_value = (PITCHBEND_CENTER + bend_offset) as u16;

        let _ = messages.push(MidiMsg::ChannelVoice {
            channel: self.ch,
            msg: ChannelVoiceMsg::PitchBend {
                bend: bend_value.clamp(PITCHBEND_MIN as u16, PITCHBEND_MAX as u16),
            },
        });

        Some(messages)
    }

    fn reset(&mut self) -> Messages {
        self.prev_base = self.from;

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
