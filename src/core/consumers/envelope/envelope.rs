use crate::core::{
    EnvelopeStageState, EnvelopeState, Modulation, OutputEvents, VoiceId, VoiceOutput,
};

use super::calc::{self, Level, StageSel};

const CC_TO_MS: u32 = 16;

/// A running envelope: the shape plus its position within the current stage.
#[derive(Debug)]
pub struct Envelope {
    voice: VoiceId,
    slot: u8,
    settings: EnvelopeState,
    stage_idx: usize,
    /// Milliseconds elapsed inside the current stage.
    stage_elapsed: u32,
    /// True once a note-triggered cycle has started.
    running: bool,
}

impl Envelope {
    pub fn new(voice: VoiceId, slot: u8, settings: EnvelopeState) -> Self {
        Self {
            voice,
            slot,
            settings,
            stage_idx: 0,
            stage_elapsed: 0,
            running: !settings.activation.note_triggered(),
        }
    }

    fn sequence(&self) -> calc::Mode {
        calc::sequence(self.settings.mode.index())
    }

    fn current_desc(&self) -> &'static calc::StageDesc {
        &self.sequence()[self.stage_idx]
    }

    fn current_params(&self) -> Option<&EnvelopeStageState> {
        match self.current_desc().sel {
            StageSel::Attack => Some(&self.settings.attack),
            StageSel::Decay => Some(&self.settings.decay),
            StageSel::Release => Some(&self.settings.release),
            StageSel::None => None,
        }
    }

    fn sustain(&self) -> f32 {
        self.settings.sustain as f32 / 127.0
    }

    fn resolve(&self, level: Level) -> f32 {
        match level {
            Level::Zero => 0.0,
            Level::Full => 1.0,
            Level::Sustain => self.sustain(),
        }
    }

    fn make_control(&self, value: f32) -> VoiceOutput {
        VoiceOutput::Control {
            voice: self.voice,
            slot: self.slot,
            value,
        }
    }

    /// Duration of the current stage in milliseconds.
    fn stage_ms(&self) -> u32 {
        self.current_params()
            .map(|s| s.duration as u32 * CC_TO_MS)
            .unwrap_or(0)
    }

    fn is_hold(&self) -> bool {
        matches!(self.current_desc().kind, calc::StageKind::Hold)
    }

    /// Applies new settings while preserving the position inside the stage.
    pub fn update(&mut self, settings: EnvelopeState) {
        let previous_ms = self.stage_ms().max(1);
        let fraction = (self.stage_elapsed as f32 / previous_ms as f32).clamp(0.0, 1.0);
        self.settings = settings;
        if self.stage_idx >= self.sequence().len() {
            self.stage_idx = 0;
            self.stage_elapsed = 0;
        }
        let new_ms = self.stage_ms();
        self.stage_elapsed = libm::roundf(fraction * new_ms as f32) as u32;
    }

    fn next_stage(&mut self) -> bool {
        self.stage_idx += 1;
        self.stage_elapsed = 0;
        let done = self.stage_idx >= self.sequence().len();
        if done {
            self.stage_idx = 0;
            if !self.settings.activation.loops() {
                self.running = false;
            }
        }
        !done
    }

    fn restart(&mut self) {
        self.stage_idx = 0;
        self.stage_elapsed = 0;
        self.running = true;
    }
}

impl Modulation for Envelope {
    fn advance(&mut self, delta_ms: u32, gated: bool) -> Option<OutputEvents> {
        if !self.running {
            return None;
        }

        let mut messages = OutputEvents::new();
        let mut remaining = delta_ms;

        loop {
            if self.is_hold() {
                if gated {
                    messages.push(self.make_control(self.hold_level())).ok();
                    return Some(messages);
                }
                if !self.next_stage() {
                    messages.push(self.make_control(0.0)).ok();
                    return Some(messages);
                }
                continue;
            }

            let duration = self.stage_ms();
            let elapsed = self.stage_elapsed.saturating_add(remaining);
            if duration == 0 || elapsed >= duration {
                remaining = elapsed.saturating_sub(duration);
                messages.push(self.make_control(self.ramp(1.0))).ok();
                if !self.next_stage() {
                    return Some(messages);
                }
                continue;
            }

            self.stage_elapsed = elapsed;
            let progress = elapsed as f32 / duration as f32;
            messages.push(self.make_control(self.ramp(progress))).ok();
            return Some(messages);
        }
    }

    fn reset(&mut self) -> OutputEvents {
        self.stage_idx = 0;
        self.stage_elapsed = 0;
        self.running = false;
        let mut messages = OutputEvents::new();
        messages.push(self.make_control(0.0)).ok();
        messages
    }
}

impl Envelope {
    fn ramp(&self, progress: f32) -> f32 {
        let calc::StageKind::Ramp { from, to } = self.current_desc().kind else {
            return self.hold_level();
        };
        let params = self
            .current_params()
            .expect("Ramp stage must have params");
        calc::envelope_level(progress, params.curve, self.resolve(from), self.resolve(to))
    }

    fn hold_level(&self) -> f32 {
        match self.current_desc().kind {
            calc::StageKind::Ramp { to, .. } => self.resolve(to),
            calc::StageKind::Hold => self.previous_ramp_end(),
        }
    }

    /// Level reached at the end of the last ramp stage before the hold.
    fn previous_ramp_end(&self) -> f32 {
        let sequence = self.sequence();
        for desc in sequence[..self.stage_idx].iter().rev() {
            if let calc::StageKind::Ramp { to, .. } = desc.kind {
                return self.resolve(to);
            }
        }
        0.0
    }
}

impl Envelope {
    /// Restarts the cycle. Called by the owning consumer on a note press.
    pub(crate) fn trigger(&mut self) {
        self.restart();
    }
}
