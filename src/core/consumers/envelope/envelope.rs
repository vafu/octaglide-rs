use heapless::Vec;

use crate::{
    core::{
        EnvelopeStageState, EnvelopeState, Modulation, Messages, VoiceId, VoiceOutput,
    },
    notes,
};

use super::calc::{self, Level, StageSel};

const CC_TO_MS: u32 = 16;

#[derive(Debug)]
pub struct Envelope {
    voice: VoiceId,
    slot: u8,
    settings: EnvelopeState,
    stage_idx: usize,
}

impl Envelope {
    pub fn new(voice: VoiceId, slot: u8, settings: EnvelopeState) -> Self {
        Self {
            voice,
            slot,
            settings,
            stage_idx: 0,
        }
    }

    fn sequence(&self, envelope: &EnvelopeState) -> calc::Mode {
        calc::sequence(envelope.mode.index())
    }

    fn current_desc(&self, envelope: &EnvelopeState) -> &'static calc::StageDesc {
        &self.sequence(envelope)[self.stage_idx]
    }

    fn current_params<'a>(&self, envelope: &'a EnvelopeState) -> Option<&'a EnvelopeStageState> {
        match self.current_desc(envelope).sel {
            StageSel::Attack => Some(&envelope.attack),
            StageSel::Decay => Some(&envelope.decay),
            StageSel::Release => Some(&envelope.release),
            StageSel::None => None,
        }
    }

    fn sustain(&self, envelope: &EnvelopeState) -> f32 {
        envelope.sustain as f32 / 127.0
    }

    fn resolve(&self, envelope: &EnvelopeState, level: Level) -> f32 {
        match level {
            Level::Zero => 0.0,
            Level::Full => 1.0,
            Level::Sustain => self.sustain(envelope),
        }
    }

    fn make_control(&self, value: f32) -> VoiceOutput {
        VoiceOutput::Control {
            voice: self.voice,
            slot: self.slot,
            value,
        }
    }
}

impl Modulation for Envelope {
    fn update(&mut self, settings: &crate::core::Settings) {
        if let crate::core::Settings::Envelope(settings) = settings {
            self.settings = *settings;
            if self.stage_idx >= self.sequence(settings).len() {
                self.stage_idx = 0;
            }
        }
    }

    async fn duration_ms(&self) -> u32 {
        let envelope = self.settings;
        let any_held = notes::read_notes(self.voice, |notes| notes.any_held())
            .await
            .unwrap_or(false);

        if matches!(self.current_desc(&envelope).kind, calc::StageKind::Hold) {
            return if any_held { u32::MAX } else { 0 };
        }
        self.current_params(&envelope)
            .map(|s| s.duration as u32 * CC_TO_MS)
            .unwrap_or(0)
    }

    async fn next_stage(&mut self) -> bool {
        let envelope = self.settings;
        self.stage_idx += 1;
        let done = self.stage_idx >= self.sequence(&envelope).len();
        if done {
            self.stage_idx = 0;
        }
        !done
    }

    async fn animate(&mut self, progress: f32, _depth: f32, _offset: f32) -> Messages {
        let envelope = self.settings;
        let calc::StageKind::Ramp { from, to } = self.current_desc(&envelope).kind else {
            return None;
        };
        let params = self
            .current_params(&envelope)
            .expect("Ramp stage must have params");
        let value = calc::envelope_level(
            progress,
            params.curve,
            self.resolve(&envelope, from),
            self.resolve(&envelope, to),
        );
        let mut messages = Vec::new();
        messages.push(self.make_control(value)).ok();
        Some(messages)
    }

    async fn reset(&mut self) -> Messages {
        self.stage_idx = 0;
        let mut messages = Vec::new();
        messages.push(self.make_control(0.0)).ok();
        Some(messages)
    }
}
