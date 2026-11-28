use crate::core::{OutputEvents, VoiceId, VoiceOutput};

use super::settings::GlideState;

const FALLBACK_BEND_RANGE_SEMITONES: f32 = 1.0;

/// A running slide from `from` to `to`.
///
/// Internal state machine: it is not a slot occupant, so it does not
/// implement `Modulation`.
#[derive(Debug)]
pub struct Slide {
    voice: VoiceId,
    settings: GlideState,
    from: u8,
    to: u8,
    active_note: u8,
    elapsed_ms: u32,
    finished: bool,
}

impl Slide {
    pub fn new(voice: VoiceId, settings: GlideState, from: u8, to: u8) -> Self {
        Self {
            voice,
            settings,
            from,
            to,
            active_note: from,
            elapsed_ms: 0,
            finished: false,
        }
    }

    pub fn update(&mut self, settings: GlideState) {
        self.settings = settings;
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    fn bend_amount(&self, semitones: f32, bend_range_semitones: f32) -> f32 {
        (semitones / bend_range_semitones).clamp(-1.0, 1.0)
    }

    /// Emits the note and bend messages for a given normalized position.
    fn emit(&mut self, progress: f32) -> OutputEvents {
        let glide = self.settings;
        let bend_range = bend_range_semitones(glide.bend_range_semitones);
        let mut messages = OutputEvents::new();

        if self.from == self.to {
            return messages;
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

            if self.active_note != 0 {
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

        messages
    }
}

impl Slide {
    pub fn advance(&mut self, delta_ms: u32, _gated: bool) -> Option<OutputEvents> {
        if self.finished {
            return None;
        }
        self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms);
        let duration = self.settings.duration_ms.max(1);
        let progress = (self.elapsed_ms as f32 / duration as f32).clamp(0.0, 1.0);
        let messages = self.emit(progress);
        if progress >= 1.0 {
            self.finished = true;
        }
        Some(messages)
    }

    pub fn reset(&mut self) -> OutputEvents {
        let mut messages = OutputEvents::new();
        messages
            .push(VoiceOutput::PitchBend {
                voice: self.voice,
                amount: 0.0,
            })
            .ok();
        self.active_note = self.from;
        self.finished = true;
        messages
    }
}

fn bend_range_semitones(value: f32) -> f32 {
    if value > 0.0 {
        value
    } else {
        FALLBACK_BEND_RANGE_SEMITONES
    }
}
