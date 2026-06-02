use crate::{
    anim::{
        animator::Cmd,
        modulators::{Envelope, Modulator},
    },
    app::Animator,
    core::event::{Event, EventPayload, EventRole, Events, control_to_u8},
    state,
    state::{EnvelopeParam, ModifierParam},
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
    async fn consume(&mut self, event: Event, out: &mut Events) {
        match event {
            Event {
                role: EventRole::Voice(voice),
                payload: EventPayload::NoteOn { .. },
            } => {
                self.animator
                    .send(Cmd::Start(Modulator::Envelope(Envelope::new(voice, 2))))
                    .await
                    .ok();
                out.push(event).ok();
            }
            Event {
                role:
                    EventRole::Modifier {
                        voice,
                        param: ModifierParam::Envelope(param),
                    },
                payload,
            } => {
                state::edit_voice(voice, |voice| match payload {
                    EventPayload::Control(value) => {
                        let value = control_to_u8(value, 0, 127);
                        match param {
                            EnvelopeParam::AttackDuration => voice.envelope.attack.duration = value,
                            EnvelopeParam::AttackCurve => voice.envelope.attack.curve = value,
                            EnvelopeParam::DecayDuration => voice.envelope.decay.duration = value,
                            EnvelopeParam::DecayCurve => voice.envelope.decay.curve = value,
                            EnvelopeParam::Sustain => voice.envelope.sustain = value,
                            EnvelopeParam::ReleaseDuration => {
                                voice.envelope.release.duration = value
                            }
                            EnvelopeParam::ReleaseCurve => voice.envelope.release.curve = value,
                            EnvelopeParam::Mode => voice.envelope.mode = value.min(2),
                        }
                    }
                    EventPayload::Delta(delta) if param == EnvelopeParam::Mode => {
                        let current = voice.envelope.mode as i16;
                        voice.envelope.mode = (current + delta).rem_euclid(3) as u8;
                    }
                    _ => {}
                })
                .await;
            }
            _ => {
                out.push(event).ok();
            }
        }
    }
}
