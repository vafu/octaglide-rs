use heapless::Vec;

use crate::{
    core::{Messages, Modulation, VoiceId, VoiceOutput},
    notes,
};

use super::settings::GlideState;

const FALLBACK_BEND_RANGE_SEMITONES: f32 = 1.0;

#[derive(Debug)]
pub struct Glide {
    voice: VoiceId,
    settings: GlideState,
    from: u8,
    to: u8,
    active_note: u8,
}

impl Glide {
    pub fn new(voice: VoiceId, settings: GlideState, from: u8, to: u8) -> Self {
        Self {
            voice,
            settings,
            from,
            to,
            active_note: from,
        }
    }

    fn bend_amount(&self, semitones: f32, bend_range_semitones: f32) -> f32 {
        (semitones / bend_range_semitones).clamp(-1.0, 1.0)
    }
}

impl Modulation for Glide {
    fn update(&mut self, settings: &crate::core::Settings) {
        if let crate::core::Settings::Glide(settings) = settings {
            self.settings = *settings;
        }
    }

    async fn duration_ms(&self) -> u32 {
        self.settings.duration_ms
    }

    async fn next_stage(&mut self) -> bool {
        false
    }

    async fn animate(&mut self, progress: f32, _depth: f32, _offset: f32) -> Messages {
        let glide = self.settings;
        let bend_range = bend_range_semitones(glide.bend_range_semitones);
        let mut messages = Vec::new();

        if self.from == self.to {
            return Some(messages);
        }

        let slide_range = self.to as f32 - self.from as f32;
        let inter_note = self.from as f32 + (slide_range * progress);

        let (new_active_note, req_ct) = {
            let dest_ct = inter_note - self.to as f32;
            if dest_ct.abs() <= bend_range {
                (self.to, dest_ct)
            } else {
                let active_ct = inter_note - self.active_note as f32;
                if active_ct.abs() <= bend_range {
                    (self.active_note, active_ct)
                } else {
                    let note = libm::roundf(inter_note) as u8;
                    (note, inter_note - note as f32)
                }
            }
        };

        if new_active_note != self.active_note {
            messages
                .push(VoiceOutput::NoteOn {
                    voice: self.voice,
                    note: new_active_note,
                    velocity: glide.note_on_velocity,
                })
                .ok();

            let active_note_held =
                notes::read_notes(self.voice, |notes| notes.is_held(self.active_note))
                    .await
                    .unwrap_or(false);
            if self.active_note != 0 && !active_note_held {
                messages
                    .push(VoiceOutput::NoteOff {
                        voice: self.voice,
                        note: self.active_note,
                        velocity: 0,
                    })
                    .ok();
            }

            self.active_note = new_active_note;
        }

        messages
            .push(VoiceOutput::PitchBend {
                voice: self.voice,
                amount: self.bend_amount(req_ct, bend_range),
            })
            .ok();

        Some(messages)
    }

    async fn reset(&mut self) -> Messages {
        let mut messages = Vec::new();

        let active_note_held =
            notes::read_notes(self.voice, |notes| notes.is_held(self.active_note))
                .await
                .unwrap_or(false);
        if self.active_note != 0 && !active_note_held {
            messages
                .push(VoiceOutput::NoteOff {
                    voice: self.voice,
                    note: self.active_note,
                    velocity: 0,
                })
                .ok();
        }

        self.active_note = self.from;
        messages
            .push(VoiceOutput::PitchBend {
                voice: self.voice,
                amount: 0.0,
            })
            .ok();

        Some(messages)
    }
}

fn bend_range_semitones(value: f32) -> f32 {
    if value > 0.0 {
        value
    } else {
        FALLBACK_BEND_RANGE_SEMITONES
    }
}
