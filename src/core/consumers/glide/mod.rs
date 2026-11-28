use crate::core::{
    Consumer, Input, Modulation, NoteKind, NoteTransition, OutputEvents, Routing, VoiceId,
};

use super::macros::consumer;
mod slide;
pub mod settings;
pub(crate) use self::slide::Slide;
pub use self::settings::{GlideMode, GlideState};

/// A voicing consumer that slides between notes with pitch bend.
#[derive(Debug)]
pub struct Glider {
    voice_id: VoiceId,
    settings: GlideState,
    engine: Option<Slide>,
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

    fn consume_input(&mut self, input: &Input, out: &mut OutputEvents) -> Routing {
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

    /// Keeps a running slide in sync with edited settings.
    fn after_control_set(&mut self) {
        if let Some(engine) = self.engine.as_mut() {
            engine.update(self.settings);
        }
        if !self.settings.enabled {
            self.engine = None;
        }
    }

    fn handle_note_on(&mut self, transition: &NoteTransition) {
        self.active_note = Some(transition.note);
        if !self.settings.enabled {
            return;
        }
        if let Some(from) = transition.previous_top {
            self.engine = Some(Slide::new(self.voice_id, self.settings, from, transition.note));
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
            self.engine = Some(Slide::new(
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

impl Modulation for Glider {
    fn advance(&mut self, delta_ms: u32, gated: bool) -> Option<OutputEvents> {
        let engine = self.engine.as_mut()?;
        let output = engine.advance(delta_ms, gated)?;
        if engine.is_finished() {
            self.engine = None;
        }
        Some(output)
    }

}

consumer! {
    Glider {
        settings: GlideState,
        wants_notes: true,
        controls: [
            0 => Enabled {
                set: |s: &mut GlideState, v: f32| s.enabled = v >= 0.5,
                get: |s: &GlideState| (s.enabled as u8) as f32,
            },
            1 => NoteOnVelocity {
                set: |s: &mut GlideState, v: f32| {
                    s.note_on_velocity = libm::roundf(1.0 + v.clamp(0.0, 1.0) * 126.0) as u8;
                },
                get: |s: &GlideState| s.note_on_velocity.saturating_sub(1) as f32 / 126.0,
            },
            2 => BendRange {
                set: |s: &mut GlideState, v: f32| {
                    s.bend_range_semitones = 1.0 + v.clamp(0.0, 1.0) * 23.0;
                },
                get: |s: &GlideState| ((s.bend_range_semitones - 1.0) / 23.0).clamp(0.0, 1.0),
            },
            3 => Duration {
                set: |s: &mut GlideState, v: f32| {
                    s.duration_ms = libm::roundf(v.clamp(0.0, 1.0) * 4000.0) as u32;
                },
                get: |s: &GlideState| s.duration_ms as f32 / 4000.0,
            },
        ],
        modes: {
            count: GlideMode::COUNT,
            get: |s: &GlideState| {
                if s.enabled { GlideMode::Enabled } else { GlideMode::Disabled }.index()
            },
            set: |s: &mut GlideState, index: u8| {
                s.enabled = matches!(GlideMode::from_index(index), GlideMode::Enabled);
            },
        },
    }
}
