use crate::{
    app::Animator,
    core::{Cmd, GlideMode, GlideState, Input, Modulator, OutputEvents, Settings, VoiceId},
};

use super::{Consumer, CONTROL_SLOTS};
mod glide;
pub(crate) use self::glide::Glide;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GliderControl {
    Enabled,
    Duration,
    BendRange,
    NoteOnVelocity,
}

#[derive(Debug)]
pub struct Glider {
    voice_id: VoiceId,
    animator: Animator,
    settings: GlideState,
}

impl Glider {
    const CONTROL_MAP: [Option<GliderControl>; CONTROL_SLOTS] = [
        Some(GliderControl::Enabled),
        Some(GliderControl::NoteOnVelocity),
        Some(GliderControl::BendRange),
        Some(GliderControl::Duration),
        None,
        None,
        None,
        None,
    ];

    pub fn new(voice_id: VoiceId, animator: Animator) -> Self {
        Self {
            voice_id,
            animator,
            settings: GlideState::INITIAL,
        }
    }

    fn set(&mut self, control: GliderControl, value: f32) {
        match control {
            GliderControl::Enabled => self.settings.enabled = value >= 0.5,
            GliderControl::Duration => {
                self.settings.duration_ms = libm::roundf(value.clamp(0.0, 1.0) * 4000.0) as u32
            }
            GliderControl::BendRange => {
                self.settings.bend_range_semitones = 1.0 + value.clamp(0.0, 1.0) * 23.0
            }
            GliderControl::NoteOnVelocity => {
                self.settings.note_on_velocity =
                    libm::roundf(1.0 + value.clamp(0.0, 1.0) * 126.0) as u8
            }
        }
    }

    fn mode(&self) -> GlideMode {
        if self.settings.enabled {
            GlideMode::Enabled
        } else {
            GlideMode::Disabled
        }
    }

    fn get(&self, control: GliderControl) -> f32 {
        match control {
            GliderControl::Enabled => (self.settings.enabled as u8) as f32,
            GliderControl::Duration => self.settings.duration_ms as f32 / 4000.0,
            GliderControl::BendRange => {
                ((self.settings.bend_range_semitones - 1.0) / 23.0).clamp(0.0, 1.0)
            }
            GliderControl::NoteOnVelocity => {
                self.settings.note_on_velocity.saturating_sub(1) as f32 / 126.0
            }
        }
    }

    async fn handle_note_on(&mut self, note: u8) -> bool {
        let Some(transition) = notes::edit_notes(self.voice_id, |notes| notes.press(note)).await
        else {
            return false;
        };
        if !self.settings.enabled {
            return false;
        }
        if let Some(from) = transition.previous_top {
            let _: Result<(), _> = self.animator
                .send(Cmd::Start(Modulator::Glide(Glide::new(
                    self.voice_id,
                    self.settings,
                    from,
                    note,
                ))))
                .await;
            true
        } else {
            false
        }
    }

    async fn handle_note_off(&mut self, note: u8) -> bool {
        let Some(release) = notes::edit_notes(self.voice_id, |notes| notes.release(note)).await
        else {
            return false;
        };
        if !self.settings.enabled {
            return false;
        }
        if let Some(release) = release
            && release.was_top
            && let Some(last_held) = release.new_top
        {
            let _: Result<(), _> = self.animator
                .send(Cmd::Start(Modulator::Glide(Glide::new(
                    self.voice_id,
                    self.settings,
                    note,
                    last_held,
                ))))
                .await;
            true
        } else {
            let _: Result<(), _> = self.animator.send(Cmd::Stop).await;
            false
        }
    }
}

impl Consumer for Glider {
    async fn consume(&mut self, input: &Input, _out: &mut OutputEvents) -> bool {
        let before = self.settings;
        let consumed = match *input {
            Input::NoteOn { voice, note, velocity } if voice == self.voice_id && velocity > 0 => {
                self.handle_note_on(note).await
            }
            Input::NoteOff { voice, note, .. } if voice == self.voice_id => {
                self.handle_note_off(note).await
            }
            Input::ControlUpdate { voice, control, value, .. } if voice == self.voice_id => {
                if let Some(mapped) = Self::CONTROL_MAP.get(control as usize).copied().flatten() {
                    self.set(mapped, value);
                    true
                } else {
                    false
                }
            }
            Input::ModeUpdate { voice, direction, .. } if voice == self.voice_id => {
                let mode = self.mode().step(direction);
                self.settings.enabled = matches!(mode, GlideMode::Enabled);
                true
            }
            _ => false,
        };
        if self.settings != before {
            let _: Result<(), _> = self.animator
                .send(Cmd::Update(Settings::Glide(self.settings)))
                .await;
        }
        consumed
    }

    fn set_control_slot(&mut self, slot: u8, value: f32) {
        if let Some(control) = Self::CONTROL_MAP.get(slot as usize).copied().flatten() {
            self.set(control, value);
        }
    }

    fn control_slot_value(&self, slot: u8) -> Option<f32> {
        Self::CONTROL_MAP
            .get(slot as usize)
            .copied()
            .flatten()
            .map(|control| self.get(control))
    }

    fn mode_index(&self) -> u8 {
        self.mode().index()
    }

    fn update_mode(&mut self, direction: i8) {
        self.settings.enabled = matches!(self.mode().step(direction), GlideMode::Enabled);
    }
}

use crate::notes;
