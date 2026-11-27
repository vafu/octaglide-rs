use crate::core::{
    Input, Modulation, NoteKind, NoteTransition, OutputEvents, Routing, VoiceId,
};
use crate::core::consumers::registry::GlideControl;

use super::Consumer;
mod glide;
pub mod settings;
pub(crate) use self::glide::Glide;
pub use self::settings::{GlideMode, GlideState};

/// A voicing consumer that slides between notes with pitch bend.
#[derive(Debug)]
pub struct Glider {
    voice_id: VoiceId,
    settings: GlideState,
    engine: Option<Glide>,
    /// The note the voice is currently sounding, if any.
    active_note: Option<u8>,
}

impl Glider {
    pub fn new(voice_id: VoiceId) -> Self {
        Self {
            voice_id,
            settings: GlideState::INITIAL,
            engine: None,
            active_note: None,
        }
    }

    fn set(&mut self, control: GlideControl, value: f32) {
        match control {
            GlideControl::Enabled => self.settings.enabled = value >= 0.5,
            GlideControl::Duration => {
                self.settings.duration_ms = libm::roundf(value.clamp(0.0, 1.0) * 4000.0) as u32
            }
            GlideControl::BendRange => {
                self.settings.bend_range_semitones = 1.0 + value.clamp(0.0, 1.0) * 23.0
            }
            GlideControl::NoteOnVelocity => {
                self.settings.note_on_velocity =
                    libm::roundf(1.0 + value.clamp(0.0, 1.0) * 126.0) as u8
            }
        }
        if let Some(engine) = self.engine.as_mut() {
            engine.update(self.settings);
        }
    }

    fn get(&self, control: GlideControl) -> f32 {
        match control {
            GlideControl::Enabled => (self.settings.enabled as u8) as f32,
            GlideControl::Duration => self.settings.duration_ms as f32 / 4000.0,
            GlideControl::BendRange => {
                ((self.settings.bend_range_semitones - 1.0) / 23.0).clamp(0.0, 1.0)
            }
            GlideControl::NoteOnVelocity => {
                self.settings.note_on_velocity.saturating_sub(1) as f32 / 126.0
            }
        }
    }

    fn handle_note_on(&mut self, transition: &NoteTransition) {
        self.active_note = Some(transition.note);
        if !self.settings.enabled {
            return;
        }
        if let Some(from) = transition.previous_top {
            self.engine = Some(Glide::new(
                self.voice_id,
                self.settings,
                from,
                transition.note,
            ));
        }
    }

    /// Returns the messages needed to release the current slide.
    ///
    /// A slide that ends without a new target must zero the pitch bend,
    /// otherwise the wire stays bent after the last note is released.
    fn handle_note_off(&mut self, transition: &NoteTransition) -> OutputEvents {
        let mut out = OutputEvents::new();
        if !self.settings.enabled {
            self.active_note = transition.new_top;
            return out;
        }
        if transition.was_top
            && let Some(last_held) = transition.new_top
        {
            self.engine = Some(Glide::new(
                self.voice_id,
                self.settings,
                transition.note,
                last_held,
            ));
        } else if let Some(mut engine) = self.engine.take() {
            for msg in engine.reset() {
                out.push(msg).ok();
            }
        }
        self.active_note = transition.new_top;
        out
    }
}

impl Consumer for Glider {
    fn consume(&mut self, input: &Input, out: &mut OutputEvents) -> Routing {
        match *input {
            Input::Note(transition @ NoteTransition { kind: NoteKind::Press, velocity, .. })
                if velocity > 0 =>
            {
                self.handle_note_on(&transition);
                Routing::Forward
            }
            Input::Note(transition @ NoteTransition { kind: NoteKind::Release, .. }) => {
                for msg in self.handle_note_off(&transition) {
                    out.push(msg).ok();
                }
                Routing::Forward
            }
            Input::ControlUpdate { control, value, .. } => {
                if Self::control_for_slot(control).is_some() {
                    self.set_control_slot(control, value);
                    Routing::Consumed
                } else {
                    Routing::Forward
                }
            }
            Input::ModeSet { index, .. } => {
                if index < GlideMode::COUNT {
                    self.set_mode(index);
                    Routing::Consumed
                } else {
                    Routing::Forward
                }
            }
            _ => Routing::Forward,
        }
    }

    fn wants_notes(&self) -> bool {
        true
    }

    fn set_control_slot(&mut self, slot: u8, value: f32) {
        if let Some(control) = Self::control_for_slot(slot) {
            self.set(control, value);
        }
    }

    fn control_slot_value(&self, slot: u8) -> Option<f32> {
        Self::control_for_slot(slot).map(|control| self.get(control))
    }

    fn mode_count(&self) -> u8 {
        GlideMode::COUNT
    }

    fn mode_index(&self) -> u8 {
        if self.settings.enabled {
            GlideMode::Enabled
        } else {
            GlideMode::Disabled
        }
        .index()
    }

    fn set_mode(&mut self, index: u8) {
        if index < GlideMode::COUNT {
            self.settings.enabled = matches!(GlideMode::from_index(index), GlideMode::Enabled);
            if !self.settings.enabled {
                self.engine = None;
            }
        }
    }

    fn output_slot(&self) -> u8 {
        0
    }
}

impl Modulation for Glider {
    fn advance(&mut self, delta_ms: u32, gated: bool) -> Option<OutputEvents> {
        let engine = self.engine.as_mut()?;
        let output = engine.advance(delta_ms, gated)?;
        if engine.is_finished() {
            self.engine = None;
        }
        Some(output)
    }

    fn reset(&mut self) -> OutputEvents {
        self.engine.as_mut().map(Modulation::reset).unwrap_or_default()
    }
}
