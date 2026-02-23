use core::sync::atomic::Ordering::Relaxed;

use midi_msg::{ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{
    anim::{
        animator::Cmd,
        modulators::{Envelope, Modulator, envelope::CONFIG},
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
        let MidiMsg::ChannelVoice { channel, msg } = event.msg else {
            return Some(event);
        };

        match msg {
            ChannelVoiceMsg::NoteOn { .. } => {
                self.animator
                    .send(Cmd::Start(Modulator::Envelope(Envelope::new(channel, 2))))
                    .await
                    .unwrap();

                Some(event)
            }
            ChannelVoiceMsg::ControlChange {
                control: ControlChange::CC { control, value },
            } => {
                match control {
                    1 => CONFIG.attack.store(value, Relaxed),
                    2 => CONFIG.decay.store(value, Relaxed),
                    3 => CONFIG.sustain.store(value, Relaxed),
                    4 => CONFIG.release.store(value, Relaxed),
                    _ => return Some(event),
                }
                None
            }
            _ => Some(event),
        }
    }
}
