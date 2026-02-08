use heapless::Vec;
use midi_msg::MidiMsg;

use crate::{
    anim::{
        animator::Cmd,
        modulators::{Envelope, Modulator},
    },
    core::{Output, consumers::ConsumeResult},
};

pub struct ModTrigger;

impl ModTrigger {
    pub fn new() -> Self {
        Self
    }
}

impl super::Consumer for ModTrigger {
    fn consume(&mut self, event: crate::core::MidiEvent) -> ConsumeResult {
        if event.synthetic {
            return ConsumeResult::ignored(event);
        }

        let MidiMsg::ChannelVoice { channel, msg } = event.msg else {
            return ConsumeResult::ignored(event);
        };

        match msg {
            midi_msg::ChannelVoiceMsg::NoteOn { .. } => {
                let mut result = Vec::new();
                let _ = result.push(Output::Animate(Cmd::Start(Modulator::Envelope(
                    Envelope::new(channel, 2),
                ))));

                ConsumeResult::Ignored(event, result)
            }

            _ => ConsumeResult::ignored(event),
        }
    }
}
