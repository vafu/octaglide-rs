use core::sync::atomic::{AtomicU8, Ordering::Relaxed};

use heapless::Vec;
use libm::powf;
use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{anim::modulators::Modulation, held_notes};

const CC_TO_MS: u32 = 16;

// ── Level ─────────────────────────────────────────────────────────────────────

/// A resolved output level for a ramp endpoint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Level {
    Zero,
    Full,
    Sustain, // resolved at runtime from EnvelopeConfig::sustain
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

/// Which named Stage parameters from EnvelopeConfig apply to this stage.
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

// ── Stage ─────────────────────────────────────────────────────────────────────

/// Mutable parameters for a named stage, shared across all modes that use it.
#[derive(Debug)]
pub struct Stage {
    pub duration: AtomicU8,
    pub curve: AtomicU8, // 0=logarithmic, 64=linear, 127=exponential
}

impl Stage {
    pub const fn new(duration: u8) -> Self {
        Self {
            duration: AtomicU8::new(duration),
            curve: AtomicU8::new(64), // linear default
        }
    }
}

// ── Mode ──────────────────────────────────────────────────────────────────────

/// A mode is simply an ordered slice of stage descriptors.
pub type Mode = &'static [StageDesc];

pub mod mode {
    use super::{Level, Mode, StageSel, StageDesc, StageKind};

    pub const AD: u8 = 0;
    pub const AR: u8 = 1;
    pub const ADSR: u8 = 2;

    const AD_SEQ: Mode = &[
        StageDesc { kind: StageKind::Ramp { from: Level::Zero, to: Level::Full    }, sel: StageSel::Attack  },
        StageDesc { kind: StageKind::Ramp { from: Level::Full, to: Level::Zero    }, sel: StageSel::Decay   },
    ];

    const AR_SEQ: Mode = &[
        StageDesc { kind: StageKind::Ramp { from: Level::Zero, to: Level::Full    }, sel: StageSel::Attack  },
        StageDesc { kind: StageKind::Hold,                                           sel: StageSel::None    },
        StageDesc { kind: StageKind::Ramp { from: Level::Full, to: Level::Zero    }, sel: StageSel::Release },
    ];

    const ADSR_SEQ: Mode = &[
        StageDesc { kind: StageKind::Ramp { from: Level::Zero,    to: Level::Full    }, sel: StageSel::Attack  },
        StageDesc { kind: StageKind::Ramp { from: Level::Full,    to: Level::Sustain }, sel: StageSel::Decay   },
        StageDesc { kind: StageKind::Hold,                                              sel: StageSel::None    },
        StageDesc { kind: StageKind::Ramp { from: Level::Sustain, to: Level::Zero   }, sel: StageSel::Release },
    ];

    pub fn sequence(m: u8) -> Mode {
        match m {
            AD   => AD_SEQ,
            AR   => AR_SEQ,
            ADSR => ADSR_SEQ,
            _    => AD_SEQ,
        }
    }
}

// ── EnvelopeConfig ────────────────────────────────────────────────────────────

/// All mutable parameters for one envelope instance.
/// Stored as a static so atomics have stable addresses.
#[derive(Debug)]
pub struct EnvelopeConfig {
    pub attack:  Stage,
    pub decay:   Stage,
    pub release: Stage,
    pub sustain: AtomicU8, // hold level, 0-127
    pub mode:    AtomicU8, // mode::AD / AR / ADSR
}

impl EnvelopeConfig {
    const fn default() -> Self {
        Self {
            attack:  Stage::new(20),
            decay:   Stage::new(20),
            release: Stage::new(20),
            sustain: AtomicU8::new(80),
            mode:    AtomicU8::new(mode::AD),
        }
    }
}

pub static CONFIGS: [EnvelopeConfig; 4] = [
    EnvelopeConfig::default(),
    EnvelopeConfig::default(),
    EnvelopeConfig::default(),
    EnvelopeConfig::default(),
];

// ── Envelope ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Envelope {
    ch:        Channel,
    cc:        u8,
    config:    &'static EnvelopeConfig,
    stage_idx: usize,
}

impl Envelope {
    pub fn new(ch: Channel, cc: u8, config: &'static EnvelopeConfig) -> Self {
        Self { ch, cc, config, stage_idx: 0 }
    }

    fn sequence(&self) -> Mode {
        mode::sequence(self.config.mode.load(Relaxed))
    }

    fn current_desc(&self) -> &'static StageDesc {
        &self.sequence()[self.stage_idx]
    }

    fn current_params(&self) -> Option<&Stage> {
        match self.current_desc().sel {
            StageSel::Attack  => Some(&self.config.attack),
            StageSel::Decay   => Some(&self.config.decay),
            StageSel::Release => Some(&self.config.release),
            StageSel::None    => None,
        }
    }

    fn sustain(&self) -> f32 {
        self.config.sustain.load(Relaxed) as f32 / 127.0
    }

    fn resolve(&self, level: Level) -> f32 {
        match level {
            Level::Zero    => 0.0,
            Level::Full    => 1.0,
            Level::Sustain => self.sustain(),
        }
    }

    fn make_cc(&self, value: u8) -> MidiMsg {
        MidiMsg::ChannelVoice {
            channel: self.ch,
            msg: ChannelVoiceMsg::ControlChange {
                control: ControlChange::CC { control: self.cc, value },
            },
        }
    }
}

impl Modulation for Envelope {
    fn duration_ms(&self) -> u32 {
        if self.current_desc().kind == StageKind::Hold {
            return if held_notes::any_held() { u32::MAX } else { 0 };
        }
        self.current_params()
            .map(|s| s.duration.load(Relaxed) as u32 * CC_TO_MS)
            .unwrap_or(0)
    }

    fn next_stage(&mut self) -> bool {
        self.stage_idx += 1;
        let done = self.stage_idx >= self.sequence().len();
        if done {
            self.stage_idx = 0;
        }
        !done
    }

    fn animate(&mut self, progress: f32, _depth: f32, _offset: f32) -> super::Messages {
        let StageKind::Ramp { from, to } = self.current_desc().kind else {
            return None; // Hold stage: stay silent
        };

        let params = self.current_params().expect("Ramp stage must have params");
        let curve_exp = powf(2.0, (params.curve.load(Relaxed) as f32 - 64.0) / 32.0);
        let p = powf(progress, curve_exp);

        let value = ((self.resolve(from) + p * (self.resolve(to) - self.resolve(from))) * 127.0) as u8;

        let mut messages = Vec::<MidiMsg, 3>::new();
        messages.push(self.make_cc(value)).ok();
        Some(messages)
    }

    fn reset(&mut self) -> super::Messages {
        self.stage_idx = 0;
        let mut messages = Vec::<MidiMsg, 3>::new();
        messages.push(self.make_cc(0)).ok();
        Some(messages)
    }
}
