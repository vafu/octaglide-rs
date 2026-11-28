//! Envelope: a modulation lane that runs an AD/AR/ADSR shape on note press.

use crate::core::{
    Consumer, Input, Modulation, NoteKind, NoteTransition, OutputEvents, Routing, Trigger,
    VoiceId, VoiceOutput,
};

pub mod calc;
pub mod params;
pub use self::params::{EnvelopeMode, EnvelopeParams, EnvelopeStageParams};

use self::calc::{Level, StageSel};

const CC_TO_MS: u32 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvelopeControl {
    ReleaseDuration,
    Sustain,
    DecayDuration,
    AttackDuration,
    ReleaseCurve,
    DecayCurve,
    AttackCurve,
    TriggerControl,
}

impl EnvelopeControl {
    const MAP: [Option<EnvelopeControl>; crate::core::CONTROL_SLOTS] = [
        Some(EnvelopeControl::ReleaseDuration),
        Some(EnvelopeControl::Sustain),
        Some(EnvelopeControl::DecayDuration),
        Some(EnvelopeControl::AttackDuration),
        Some(EnvelopeControl::ReleaseCurve),
        Some(EnvelopeControl::DecayCurve),
        Some(EnvelopeControl::AttackCurve),
        Some(EnvelopeControl::TriggerControl),
    ];

    fn for_slot(slot: u8) -> Option<Self> {
        Self::MAP.get(slot as usize).copied().flatten()
    }
}

/// A modulation lane that runs an envelope on note press.
///
/// Owns both the user parameters and the running shape, if any.
#[derive(Debug)]
pub struct ModTrigger {
    voice: VoiceId,
    output_slot: u8,
    params: EnvelopeParams,
    /// The in-progress envelope, if a cycle is running.
    engine: Option<Envelope>,
}

/// Position of the envelope currently in flight.
#[derive(Debug)]
struct Envelope {
    /// Index into the mode's stage sequence.
    stage_idx: usize,
    /// Milliseconds elapsed inside the current stage.
    stage_elapsed: u32,
    /// True once a cycle has started.
    running: bool,
}

impl ModTrigger {
    pub fn new(voice: VoiceId, output_slot: u8) -> Self {
        let params = EnvelopeParams::INITIAL;
        Self {
            voice,
            output_slot,
            params,
            engine: params.trigger.note_triggered().then(Self::fresh_engine),
        }
    }

    fn fresh_engine() -> Envelope {
        Envelope {
            stage_idx: 0,
            stage_elapsed: 0,
            running: true,
        }
    }

    fn set(&mut self, control: EnvelopeControl, value: f32) {
        let value = u8_from_normalized(value);
        match control {
            EnvelopeControl::AttackDuration => self.params.attack.duration = value,
            EnvelopeControl::DecayDuration => self.params.decay.duration = value,
            EnvelopeControl::Sustain => self.params.sustain = value,
            EnvelopeControl::ReleaseDuration => self.params.release.duration = value,
            EnvelopeControl::AttackCurve => self.params.attack.curve = value,
            EnvelopeControl::DecayCurve => self.params.decay.curve = value,
            EnvelopeControl::ReleaseCurve => self.params.release.curve = value,
            EnvelopeControl::TriggerControl => {
                self.params.trigger = trigger_from_normalized(value as f32 / 127.0);
                if !self.params.trigger.note_triggered() && self.engine.is_none() {
                    self.engine = Some(Self::fresh_engine());
                }
            }
        }
        self.rescale_position();
    }

    fn get(&self, control: EnvelopeControl) -> f32 {
        match control {
            EnvelopeControl::AttackDuration => normalized_from_u8(self.params.attack.duration),
            EnvelopeControl::DecayDuration => normalized_from_u8(self.params.decay.duration),
            EnvelopeControl::Sustain => normalized_from_u8(self.params.sustain),
            EnvelopeControl::ReleaseDuration => normalized_from_u8(self.params.release.duration),
            EnvelopeControl::AttackCurve => normalized_from_u8(self.params.attack.curve),
            EnvelopeControl::DecayCurve => normalized_from_u8(self.params.decay.curve),
            EnvelopeControl::ReleaseCurve => normalized_from_u8(self.params.release.curve),
            EnvelopeControl::TriggerControl => {
                self.params.trigger.index() as f32 / (Trigger::COUNT - 1) as f32
            }
        }
    }

    /// Keeps the running envelope's relative position when a stage length changes.
    fn rescale_position(&mut self) {
        let Some(engine) = self.engine.as_ref() else {
            return;
        };
        let stage_count = self.sequence().len();
        let stage_idx = if engine.stage_idx >= stage_count {
            0
        } else {
            engine.stage_idx
        };
        let mut probe = Envelope {
            stage_idx,
            stage_elapsed: engine.stage_elapsed,
            running: engine.running,
        };
        let previous_ms = self.stage_ms(&probe).max(1);
        let fraction = (probe.stage_elapsed as f32 / previous_ms as f32).clamp(0.0, 1.0);
        probe.stage_elapsed = 0;
        let new_ms = self.stage_ms(&probe);
        let elapsed = libm::roundf(fraction * new_ms as f32) as u32;

        if let Some(engine) = self.engine.as_mut() {
            engine.stage_idx = stage_idx;
            engine.stage_elapsed = elapsed;
        }
    }

    fn sequence(&self) -> calc::Mode {
        calc::sequence(self.params.mode.index())
    }

    fn stage_desc(&self, engine: &Envelope) -> &'static calc::StageDesc {
        &self.sequence()[engine.stage_idx]
    }

    fn stage_params(&self, engine: &Envelope) -> Option<&EnvelopeStageParams> {
        match self.stage_desc(engine).sel {
            StageSel::Attack => Some(&self.params.attack),
            StageSel::Decay => Some(&self.params.decay),
            StageSel::Release => Some(&self.params.release),
            StageSel::None => None,
        }
    }

    fn stage_ms(&self, engine: &Envelope) -> u32 {
        self.stage_params(engine)
            .map(|stage| stage.duration as u32 * CC_TO_MS)
            .unwrap_or(0)
    }

    fn is_hold(&self, engine: &Envelope) -> bool {
        matches!(self.stage_desc(engine).kind, calc::StageKind::Hold)
    }

    fn sustain(&self) -> f32 {
        self.params.sustain as f32 / 127.0
    }

    fn resolve(&self, level: Level) -> f32 {
        match level {
            Level::Zero => 0.0,
            Level::Full => 1.0,
            Level::Sustain => self.sustain(),
        }
    }

    /// Level reached at the end of the last ramp before the current hold.
    fn hold_level(&self, engine: &Envelope) -> f32 {
        match self.stage_desc(engine).kind {
            calc::StageKind::Ramp { to, .. } => self.resolve(to),
            calc::StageKind::Hold => self
                .sequence()
                .iter()
                .take(engine.stage_idx)
                .rev()
                .find_map(|desc| match desc.kind {
                    calc::StageKind::Ramp { to, .. } => Some(self.resolve(to)),
                    calc::StageKind::Hold => None,
                })
                .unwrap_or(0.0),
        }
    }

    fn ramp(&self, engine: &Envelope, progress: f32) -> f32 {
        let calc::StageKind::Ramp { from, to } = self.stage_desc(engine).kind else {
            return self.hold_level(engine);
        };
        let stage = self
            .stage_params(engine)
            .expect("ramp stage must have params");
        calc::envelope_level(
            progress,
            stage.curve,
            self.resolve(from),
            self.resolve(to),
        )
    }

    fn control(&self, value: f32) -> VoiceOutput {
        VoiceOutput::Control {
            voice: self.voice,
            slot: self.output_slot,
            value,
        }
    }

    fn advance_stage(&mut self, engine: &mut Envelope) -> bool {
        engine.stage_idx += 1;
        engine.stage_elapsed = 0;
        let done = engine.stage_idx >= self.sequence().len();
        if done {
            engine.stage_idx = 0;
            if !self.params.trigger.loops() {
                engine.running = false;
            }
        }
        !done
    }

    fn advance_engine(&mut self, delta_ms: u32, gated: bool) -> Option<OutputEvents> {
        if !self.engine.as_ref()?.running {
            return None;
        }

        let mut messages = OutputEvents::new();
        let mut remaining = delta_ms;

        loop {
            let engine = self.engine.as_ref()?;

            if self.is_hold(engine) {
                let level = self.hold_level(engine);
                if gated {
                    messages.push(self.control(level)).ok();
                    return Some(messages);
                }
                let mut engine = self.engine.take()?;
                let more = self.advance_stage(&mut engine);
                self.engine = Some(engine);
                if !more {
                    messages.push(self.control(0.0)).ok();
                    return Some(messages);
                }
                continue;
            }

            let duration = self.stage_ms(engine);
            let elapsed = engine.stage_elapsed.saturating_add(remaining);
            if duration == 0 || elapsed >= duration {
                let level = self.ramp(engine, 1.0);
                remaining = elapsed.saturating_sub(duration);
                messages.push(self.control(level)).ok();
                let mut engine = self.engine.take()?;
                let more = self.advance_stage(&mut engine);
                self.engine = Some(engine);
                if !more {
                    return Some(messages);
                }
                continue;
            }

            let progress = elapsed as f32 / duration as f32;
            let level = self.ramp(engine, progress);
            if let Some(engine) = self.engine.as_mut() {
                engine.stage_elapsed = elapsed;
            }
            messages.push(self.control(level)).ok();
            return Some(messages);
        }
    }
    /// Applies a control by slot. Out-of-range or padding slots are ignored.
    pub fn set_control(&mut self, slot: u8, value: f32) {
        if let Some(control) = EnvelopeControl::for_slot(slot) {
            self.set(control, value);
        }
    }

}

impl Consumer for ModTrigger {
    fn consume(&mut self, input: &Input, _out: &mut OutputEvents) -> Routing {
        match *input {
            Input::Note(NoteTransition { kind: NoteKind::Press, velocity, .. })
                if velocity > 0 =>
            {
                if self.params.trigger.note_triggered() {
                    self.engine = Some(Self::fresh_engine());
                }
                Routing::Forward
            }
            Input::ControlUpdate { control, value, .. } => {
                if EnvelopeControl::for_slot(control).is_some() {
                    self.set_control(control, value);
                    Routing::Consumed
                } else {
                    Routing::Forward
                }
            }
            Input::ModeSet { index, .. } => {
                if index < EnvelopeMode::COUNT {
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
        EnvelopeControl::for_slot(slot).map(|control| self.get(control))
    }

    fn mode_count(&self) -> u8 {
        EnvelopeMode::COUNT
    }

    fn mode_index(&self) -> u8 {
        self.params.mode.index()
    }

    fn set_mode(&mut self, index: u8) {
        if index < EnvelopeMode::COUNT {
            self.params.mode = EnvelopeMode::from_index(index);
            let stage_count = self.sequence().len();
            if let Some(engine) = self.engine.as_mut()
                && engine.stage_idx >= stage_count
            {
                engine.stage_idx = 0;
            }
        }
    }

    fn output_slot(&self) -> u8 {
        self.output_slot
    }
}

impl Modulation for ModTrigger {
    fn advance(&mut self, delta_ms: u32, gated: bool) -> Option<OutputEvents> {
        self.advance_engine(delta_ms, gated)
    }
}

fn u8_from_normalized(value: f32) -> u8 {
    libm::roundf(value.clamp(0.0, 1.0) * 127.0) as u8
}

fn normalized_from_u8(value: u8) -> f32 {
    value as f32 / 127.0
}

/// Maps a normalized value onto the three trigger variants.
///
/// A discrete control spans the full 0..=1 range so each variant sits at a
/// round value (0.0, 0.5, 1.0) rather than being squeezed into 7 bits.
fn trigger_from_normalized(value: f32) -> Trigger {
    let steps = (Trigger::COUNT - 1) as f32;
    Trigger::from_index(libm::roundf(value.clamp(0.0, 1.0) * steps) as u8)
}
