use log::info;

use crate::{
    anim::{
        animator::Cmd,
        modulators::{Glide, Modulator},
    },
    app::Animator,
    core::event::{Event, EventPayload, EventRole, Events, control_to_u8, control_to_u32},
    state::{self, GlideParam, ModifierParam},
};

#[derive(Debug)]
pub struct Glider {
    animator: Animator,
}

impl Glider {
    pub fn new(animator: Animator) -> Self {
        Glider { animator }
    }

    async fn handle_note_on(&mut self, event: Event, out: &mut Events, note: u8) {
        let EventRole::Voice(voice) = event.role else {
            out.push(event).ok();
            return;
        };

        let Some((enabled, transition)) = state::edit_voice(voice, |voice| {
            (voice.glide.enabled, voice.held_notes.press(note))
        })
        .await
        else {
            out.push(event).ok();
            return;
        };

        if !enabled {
            out.push(event).ok();
            return;
        }

        if let Some(from) = transition.previous_top {
            self.animator
                .send(Cmd::Start(Modulator::Glide(Glide::new(voice, from, note))))
                .await
                .ok();
        } else {
            out.push(event).ok();
        }
    }

    async fn handle_note_off(&mut self, event: Event, out: &mut Events, note: u8) {
        let EventRole::Voice(voice) = event.role else {
            out.push(event).ok();
            return;
        };

        let Some((enabled, release)) = state::edit_voice(voice, |voice| {
            (voice.glide.enabled, voice.held_notes.release(note))
        })
        .await
        else {
            out.push(event).ok();
            return;
        };

        if !enabled {
            out.push(event).ok();
            return;
        }

        if let Some(release) = release
            && release.was_top
            && let Some(last_held) = release.new_top
        {
            self.animator
                .send(Cmd::Start(Modulator::Glide(Glide::new(
                    voice, note, last_held,
                ))))
                .await
                .ok();
            info!("sliding from {} back to {}", note, last_held);
        } else {
            info!("canceling anim");
            self.animator.send(Cmd::Stop).await.ok();
            out.push(event).ok();
        }
    }
}

impl super::Consumer for Glider {
    async fn consume(&mut self, event: Event, out: &mut Events) {
        match event {
            Event {
                role:
                    EventRole::Modifier {
                        voice,
                        param: ModifierParam::Glide(param),
                    },
                payload,
            } => {
                state::edit_voice(voice, |voice| match payload {
                    EventPayload::Control(value) => match param {
                        GlideParam::Enabled => voice.glide.enabled = value >= u16::MAX / 2,
                        GlideParam::Duration => {
                            voice.glide.duration_ms = control_to_u32(value, 0, 4000);
                        }
                        GlideParam::BendRange => {
                            voice.glide.bend_range_semitones = control_to_u32(value, 1, 24) as f32;
                        }
                        GlideParam::NoteOnVelocity => {
                            voice.glide.note_on_velocity = control_to_u8(value, 1, 127);
                        }
                    },
                    EventPayload::Delta(delta) => match param {
                        GlideParam::Enabled => {
                            if delta != 0 {
                                voice.glide.enabled = !voice.glide.enabled;
                            }
                        }
                        GlideParam::Duration => {
                            let current = voice.glide.duration_ms as i32;
                            voice.glide.duration_ms =
                                (current + delta as i32 * 10).clamp(0, 4000) as u32;
                        }
                        GlideParam::BendRange => {
                            let current = voice.glide.bend_range_semitones as i32;
                            voice.glide.bend_range_semitones =
                                (current + delta as i32).clamp(1, 24) as f32;
                        }
                        GlideParam::NoteOnVelocity => {
                            let current = voice.glide.note_on_velocity as i16;
                            voice.glide.note_on_velocity = (current + delta).clamp(1, 127) as u8;
                        }
                    },
                    _ => {}
                })
                .await;
            }
            event @ Event {
                role: EventRole::Voice(_),
                payload: EventPayload::NoteOn { note, .. },
                ..
            } => {
                self.handle_note_on(event, out, note).await;
            }
            event @ Event {
                role: EventRole::Voice(_),
                payload: EventPayload::NoteOff { note, .. },
                ..
            } => {
                self.handle_note_off(event, out, note).await;
            }
            _ => {
                out.push(event).ok();
            }
        }
    }
}
