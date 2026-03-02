use core::sync::atomic::Ordering::Relaxed;

use midi_msg::{ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{
    anim::{
        animator::Cmd,
        modulators::{Envelope, Modulator, envelope::CONFIGS},
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
                    .send(Cmd::Start(Modulator::Envelope(Envelope::new(
                        channel,
                        2,
                        &CONFIGS[0],
                    ))))
                    .await
                    .unwrap();
                Some(event)
            }
            ChannelVoiceMsg::ControlChange {
                control: ControlChange::CC { control, value },
            } => {
                match control {
                    // CC 1-5: envelope parameters for CONFIGS[0]
                    1 => CONFIGS[0].attack.store(value, Relaxed),
                    2 => CONFIGS[0].decay.store(value, Relaxed),
                    3 => CONFIGS[0].release.store(value, Relaxed),
                    4 => CONFIGS[0].sustain.store(value, Relaxed),
                    5 => CONFIGS[0].mode.store(value.min(2), Relaxed),
                    _ => return Some(event),
                }
                None
            }
            _ => Some(event),
        }
    }
}
