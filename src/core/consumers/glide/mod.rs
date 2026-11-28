//! Glide: a voicing consumer that slides between notes with pitch bend.

use crate::core::{
    Consumer, Input, Modulation, NoteKind, NoteTransition, OutputEvents, Routing, VoiceId,
    VoiceOutput,
};

pub mod params;
pub use self::params::{GlideMode, GlideParams};

const FALLBACK_BEND_RANGE_SEMITONES: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlideControl {
    Enabled,
    NoteOnVelocity,
    BendRange,
    Duration,
}

impl GlideControl {
    const MAP: [Option<GlideControl>; crate::core::CONTROL_SLOTS] = [
        Some(GlideControl::Enabled),
        Some(GlideControl::NoteOnVelocity),
        Some(GlideControl::BendRange),
        Some(GlideControl::Duration),
        None,
        None,
        None,
        None,
    ];

    fn for_slot(slot: u8) -> Option<Self> {
        Self::MAP.get(slot as usize).copied().flatten()
    }
}

/// A voicing consumer that slides between notes with pitch bend.
///
/// Owns both the user parameters and the running slide, if any.
#[derive(Debug)]
pub struct Glider {
    voice: VoiceId,
    params: GlideParams,
    /// The in-progress slide, from one note to another.
    slide: Option<Slide>,
}

/// Position of the slide currently in flight.
#[derive(Debug)]
struct Slide {
    from: u8,
    to: u8,
    /// The note actually sounding right now.
    active_note: u8,
    elapsed_ms: u32,
}

impl Slide {
    fn new(from: u8, to: u8) -> Self {
        Self {
            from,
            to,
            active_note: from,
            elapsed_ms: 0,
        }
    }
}

impl Glider {
    pub fn new(voice: VoiceId) -> Self {
        Self {
            voice,
            params: GlideParams::INITIAL,
            slide: None,
        }
    }

    fn set(&mut self, control: GlideControl, value: f32) {
        match control {
            GlideControl::Enabled => self.params.enabled = value >= 0.5,
            GlideControl::NoteOnVelocity => {
                self.params.note_on_velocity =
                    libm::roundf(1.0 + value.clamp(0.0, 1.0) * 126.0) as u8
            }
            GlideControl::BendRange => {
                self.params.bend_range_semitones = 1.0 + value.clamp(0.0, 1.0) * 23.0
            }
            GlideControl::Duration => {
                self.params.duration_ms = libm::roundf(value.clamp(0.0, 1.0) * 4000.0) as u32
            }
        }
        if !self.params.enabled {
            self.slide = None;
        }
    }

    fn get(&self, control: GlideControl) -> f32 {
        match control {
            GlideControl::Enabled => (self.params.enabled as u8) as f32,
            GlideControl::NoteOnVelocity => {
                self.params.note_on_velocity.saturating_sub(1) as f32 / 126.0
            }
            GlideControl::BendRange => {
                ((self.params.bend_range_semitones - 1.0) / 23.0).clamp(0.0, 1.0)
            }
            GlideControl::Duration => self.params.duration_ms as f32 / 4000.0,
        }
    }

    fn mode(&self) -> GlideMode {
        if self.params.enabled {
            GlideMode::Enabled
        } else {
            GlideMode::Disabled
        }
    }

    fn bend_range(&self) -> f32 {
        let range = self.params.bend_range_semitones;
        if range > 0.0 {
            range
        } else {
            FALLBACK_BEND_RANGE_SEMITONES
        }
    }

    fn handle_note_on(&mut self, transition: &NoteTransition) {
        if !self.params.enabled {
            return;
        }
        if let Some(from) = transition.previous_top {
            self.slide = Some(Slide::new(from, transition.note));
        }
    }

    /// Returns the messages needed to release the current slide.
    ///
    /// A slide that ends without a new target must zero the pitch bend,
    /// otherwise the wire stays bent after the last note is released.
    fn handle_note_off(&mut self, transition: &NoteTransition) -> OutputEvents {
        let mut out = OutputEvents::new();
        if !self.params.enabled {
            return out;
        }
        if transition.was_top
            && let Some(last_held) = transition.new_top
        {
            self.slide = Some(Slide::new(transition.note, last_held));
        } else if self.slide.take().is_some() {
            out.push(VoiceOutput::PitchBend {
                voice: self.voice,
                amount: 0.0,
            })
            .ok();
        }
        out
    }

    /// Emits note and bend messages for the slide at `progress` (0..=1).
    fn emit(&mut self, progress: f32) -> OutputEvents {
        let bend_range = self.bend_range();
        let velocity = self.params.note_on_velocity;
        let voice = self.voice;
        let Some(slide) = self.slide.as_mut() else {
            return OutputEvents::new();
        };
        let mut messages = OutputEvents::new();
        if slide.from == slide.to {
            return messages;
        }

        let slide_range = slide.to as f32 - slide.from as f32;
        let inter_note = slide.from as f32 + (slide_range * progress);

        let (new_active_note, req_ct) = {
            let dest_ct = inter_note - slide.to as f32;
            if dest_ct.abs() <= bend_range {
                (slide.to, dest_ct)
            } else {
                let active_ct = inter_note - slide.active_note as f32;
                if active_ct.abs() <= bend_range {
                    (slide.active_note, active_ct)
                } else {
                    let note = libm::roundf(inter_note) as u8;
                    (note, inter_note - note as f32)
                }
            }
        };

        if new_active_note != slide.active_note {
            messages
                .push(VoiceOutput::NoteOn {
                    voice,
                    note: new_active_note,
                    velocity,
                })
                .ok();
            if slide.active_note != 0 {
                messages
                    .push(VoiceOutput::NoteOff {
                        voice,
                        note: slide.active_note,
                        velocity: 0,
                    })
                    .ok();
            }
            slide.active_note = new_active_note;
        }

        messages
            .push(VoiceOutput::PitchBend {
                voice,
                amount: (req_ct / bend_range).clamp(-1.0, 1.0),
            })
            .ok();
        messages
    }
    /// Applies a control by slot. Out-of-range or padding slots are ignored.
    pub fn set_control(&mut self, slot: u8, value: f32) {
        if let Some(control) = GlideControl::for_slot(slot) {
            self.set(control, value);
        }
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
                if GlideControl::for_slot(control).is_some() {
                    self.set_control(control, value);
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


    fn control_slot_value(&self, slot: u8) -> Option<f32> {
        GlideControl::for_slot(slot).map(|control| self.get(control))
    }

    fn mode_count(&self) -> u8 {
        GlideMode::COUNT
    }

    fn mode_index(&self) -> u8 {
        self.mode().index()
    }

    fn set_mode(&mut self, index: u8) {
        if index < GlideMode::COUNT {
            self.params.enabled = matches!(GlideMode::from_index(index), GlideMode::Enabled);
            if !self.params.enabled {
                self.slide = None;
            }
        }
    }
}

impl Modulation for Glider {
    fn advance(&mut self, delta_ms: u32, _gated: bool) -> Option<OutputEvents> {
        let slide = self.slide.as_mut()?;
        slide.elapsed_ms = slide.elapsed_ms.saturating_add(delta_ms);
        let duration = self.params.duration_ms.max(1);
        let progress = (slide.elapsed_ms as f32 / duration as f32).clamp(0.0, 1.0);
        let messages = self.emit(progress);
        if progress >= 1.0 {
            self.slide = None;
        }
        Some(messages)
    }
}
