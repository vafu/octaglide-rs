use midi_msg::MidiMsg;

use crate::{
    anim::{
        animator::Cmd,
        modulators::{Envelope, Modulator},
    },
    app::Animator,
};

#[derive(Debug)]
pub struct ModTrigger {
    animator: Animator,
}

impl ModTrigger {
    pub fn new(animator: Animator) -> Self {
        Self { animator }
    }
}

impl super::Consumer for ModTrigger {
    async fn consume(&mut self, event: crate::core::MidiEvent) -> Option<crate::core::MidiEvent> {
        if event.synthetic {
            return Some(event);
        }

        let MidiMsg::ChannelVoice { channel, msg } = event.msg else {
            return Some(event);
        };

        match msg {
            midi_msg::ChannelVoiceMsg::NoteOn { .. } => {
                self.animator
                    .send(Cmd::Start(Modulator::Envelope(Envelope::new(channel, 2))))
                    .await
                    .unwrap();

                Some(event)
            }

            _ => Some(event),
        }
    }
}
