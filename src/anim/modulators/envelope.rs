use heapless::Vec;
use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{
    anim::modulators::Modulation,
    core::{EnvelopeStageState, EnvelopeState, VoiceId},
    notes,
};

const CC_TO_MS: u32 = 16;

// ── Level ─────────────────────────────────────────────────────────────────────

/// A resolved output level for a ramp endpoint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Level {
    Zero,
    Full,
    Sustain, // resolved from this animation's current settings
}

// ── StageKind ─────────────────────────────────────────────────────────────────

/// What a stage does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StageKind {
    /// Linear (or curved) ramp between two levels.
    Ramp { from: Level, to: Level },
    /// Hold at sustain level until all notes are released, then advance.
    Hold,
}

// ── StageSel ──────────────────────────────────────────────────────────────────

/// Which named envelope parameters apply to this stage.
#[derive(Clone, Copy, Debug)]
pub enum StageSel {
    Attack,
    Decay,
    Release,
    None, // Hold stages have no configurable duration/curve
}

// ── StageDesc ─────────────────────────────────────────────────────────────────

/// Immutable description of one step in a mode's sequence.
/// Defined once per mode; contains no mutable state.
#[derive(Clone, Copy)]
pub struct StageDesc {
    pub kind: StageKind,
    pub sel: StageSel,
}

// ── Mode ──────────────────────────────────────────────────────────────────────

/// A mode is simply an ordered slice of stage descriptors.
pub type Mode = &'static [StageDesc];

pub mod mode {
    use super::{Level, Mode, StageDesc, StageKind, StageSel};

    pub const AD: u8 = 0;
    pub const AR: u8 = 1;
    pub const ADSR: u8 = 2;

    const AD_SEQ: Mode = &[
        StageDesc {
            kind: StageKind::Ramp {
                from: Level::Zero,
                to: Level::Full,
            },
            sel: StageSel::Attack,
        },
        StageDesc {
            kind: StageKind::Ramp {
                from: Level::Full,
                to: Level::Zero,
            },
            sel: StageSel::Decay,
        },
    ];

    const AR_SEQ: Mode = &[
        StageDesc {
            kind: StageKind::Ramp {
                from: Level::Zero,
                to: Level::Full,
            },
            sel: StageSel::Attack,
        },
        StageDesc {
            kind: StageKind::Hold,
            sel: StageSel::None,
        },
        StageDesc {
            kind: StageKind::Ramp {
                from: Level::Full,
                to: Level::Zero,
            },
            sel: StageSel::Release,
        },
    ];

    const ADSR_SEQ: Mode = &[
        StageDesc {
            kind: StageKind::Ramp {
                from: Level::Zero,
                to: Level::Full,
            },
            sel: StageSel::Attack,
        },
        StageDesc {
            kind: StageKind::Ramp {
                from: Level::Full,
                to: Level::Sustain,
            },
            sel: StageSel::Decay,
        },
        StageDesc {
            kind: StageKind::Hold,
            sel: StageSel::None,
        },
        StageDesc {
            kind: StageKind::Ramp {
                from: Level::Sustain,
                to: Level::Zero,
            },
            sel: StageSel::Release,
        },
    ];

    pub fn sequence(m: u8) -> Mode {
        match m {
            AD => AD_SEQ,
            AR => AR_SEQ,
            ADSR => ADSR_SEQ,
            _ => AD_SEQ,
        }
    }
}

// ── Envelope ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Envelope {
    voice: VoiceId,
    channel: Channel,
    settings: EnvelopeState,
    cc: u8,
    stage_idx: usize,
}

impl Envelope {
    pub fn new(voice: VoiceId, channel: Channel, cc: u8, settings: EnvelopeState) -> Self {
        Self {
            voice,
            channel,
            settings,
            cc,
            stage_idx: 0,
        }
    }

    fn sequence(&self, envelope: &EnvelopeState) -> Mode {
        mode::sequence(envelope.mode)
    }

    fn current_desc(&self, envelope: &EnvelopeState) -> &'static StageDesc {
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

    fn make_cc(&self, channel: Channel, value: u8) -> MidiMsg {
        MidiMsg::ChannelVoice {
            channel,
            msg: ChannelVoiceMsg::ControlChange {
                control: ControlChange::CC {
                    control: self.cc,
                    value,
                },
            },
        }
    }
}

impl Modulation for Envelope {
    fn update(&mut self, settings: &crate::anim::Settings) {
        if let crate::anim::Settings::Envelope(settings) = settings {
            // A mode switch may select a shorter stage sequence. Retain the
            // current stage when valid and restart the sequence index otherwise.
            self.settings = *settings;
            if self.stage_idx >= mode::sequence(settings.mode).len() {
                self.stage_idx = 0;
            }
        }
    }

    async fn duration_ms(&self) -> u32 {
        let envelope = self.settings;
        let any_held = notes::read_notes(self.voice, |notes| notes.any_held())
            .await
            .unwrap_or(false);

        if self.current_desc(&envelope).kind == StageKind::Hold {
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

    async fn animate(&mut self, progress: f32, _depth: f32, _offset: f32) -> super::Messages {
        let (channel, envelope) = (self.channel, self.settings);

        let StageKind::Ramp { from, to } = self.current_desc(&envelope).kind else {
            return None; // Hold stage: stay silent
        };

        let params = self
            .current_params(&envelope)
            .expect("Ramp stage must have params");
        let value = (super::envelope_level(
            progress,
            params.curve,
            self.resolve(&envelope, from),
            self.resolve(&envelope, to),
        ) * 127.0) as u8;

        let mut messages = Vec::<MidiMsg, 3>::new();
        messages.push(self.make_cc(channel, value)).ok();
        Some(messages)
    }

    async fn reset(&mut self) -> super::Messages {
        let channel = self.channel;
        self.stage_idx = 0;
        let mut messages = Vec::<MidiMsg, 3>::new();
        messages.push(self.make_cc(channel, 0)).ok();
        Some(messages)
    }
}
