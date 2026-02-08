use heapless::Vec;
use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::anim::modulators::Modulation;

#[derive(Debug)]
pub struct Envelope {
    ch: Channel,
    cc: u8,
}

impl Envelope {
    pub fn new(ch: Channel, cc: u8) -> Self {
        Self { ch, cc }
    }
}

impl Modulation for Envelope {
    fn animate(&mut self, progress: f32, _depth: f32, _offset: f32) -> super::Messages {
        let mut messages = Vec::<MidiMsg, 3>::new();

        // Triangle wave: up from 0.0-0.5, down from 0.5-1.0
        let value = if progress <= 0.5 {
            // Rising: 0 to 127
            (progress * 2.0 * 127.0) as u8
        } else {
            // Falling: 127 to 0
            ((1.0 - progress) * 2.0 * 127.0) as u8
        };

        // Send CC message
        let _ = messages.push(MidiMsg::ChannelVoice {
            channel: self.ch,
            msg: ChannelVoiceMsg::ControlChange {
                control: ControlChange::CC {
                    control: self.cc,
                    value,
                },
            },
        });

        Some(messages)
    }

    fn reset(&mut self) -> super::Messages {
        let mut messages = Vec::<MidiMsg, 3>::new();

        // Reset CC to 0
        let _ = messages.push(MidiMsg::ChannelVoice {
            channel: self.ch,
            msg: ChannelVoiceMsg::ControlChange {
                control: ControlChange::CC {
                    control: self.cc,
                    value: 0,
                },
            },
        });

        Some(messages)
    }
}
