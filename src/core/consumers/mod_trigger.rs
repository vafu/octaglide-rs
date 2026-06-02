use midi_msg::{ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{
    anim::{
        animator::Cmd,
        modulators::{Envelope, Modulator},
    },
    app::Animator,
    state,
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
                if !state::has_voice(channel).await {
                    return Some(event);
                }
                self.animator
                    .send(Cmd::Start(Modulator::Envelope(Envelope::new(channel, 2))))
                    .await
                    .unwrap();
                Some(event)
            }
            ChannelVoiceMsg::ControlChange {
                control: ControlChange::CC { control, value },
            } => {
                let consumed = state::edit_voice(channel, |voice| {
                    let envelope = &mut voice.envelope;
                    match control {
                        // Temporary: route CC edits to the incoming event channel's voice.
                        1 => envelope.attack.duration = value,
                        2 => envelope.decay.duration = value,
                        3 => envelope.release.duration = value,
                        4 => envelope.sustain = value,
                        5 => envelope.mode = value.min(2),
                        _ => return false,
                    }
                    true
                })
                .await
                .unwrap_or(false);
                if !consumed {
                    return Some(event);
                }
                None
            }
            _ => Some(event),
        }
    }
}
