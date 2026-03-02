use core::sync::atomic::{AtomicU8, Ordering::Relaxed};

use heapless::Vec;
use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{anim::modulators::Modulation, held_notes};

const CC_TO_MS: u32 = 16;

/// Envelope operating mode.
/// Stored as u8 in `EnvelopeConfig::mode`.
pub mod mode {
    pub const AD: u8 = 0;   // Attack → Decay
    pub const AR: u8 = 1;   // Attack → Hold → Release
    pub const ADSR: u8 = 2; // Attack → Decay → Hold → Release
}

#[derive(Debug)]
pub struct EnvelopeConfig {
    pub attack: AtomicU8,
    /// Decay duration (AD stage 1, ADSR stage 1).
    pub decay: AtomicU8,
    /// Release duration (AR stage 2, ADSR stage 3).
    pub release: AtomicU8,
    /// Sustain level 0-127 (ADSR hold value; AR hold is always 127).
    pub sustain: AtomicU8,
    /// Envelope mode: 0=AD, 1=AR, 2=ADSR  (see `mode` sub-module).
    pub mode: AtomicU8,
}

impl EnvelopeConfig {
    const fn default() -> Self {
        Self {
            attack: AtomicU8::new(20),
            decay: AtomicU8::new(20),
            release: AtomicU8::new(20),
            sustain: AtomicU8::new(80),
            mode: AtomicU8::new(mode::AD),
        }
    }
}

pub static CONFIGS: [EnvelopeConfig; 4] = [
    EnvelopeConfig::default(),
    EnvelopeConfig::default(),
    EnvelopeConfig::default(),
    EnvelopeConfig::default(),
];

#[derive(Debug)]
pub struct Envelope {
    ch: Channel,
    cc: u8,
    config: &'static EnvelopeConfig,
    stage: u8,
}

impl Envelope {
    pub fn new(ch: Channel, cc: u8, config: &'static EnvelopeConfig) -> Self {
        Self { ch, cc, config, stage: 0 }
    }

    fn current_mode(&self) -> u8 {
        self.config.mode.load(Relaxed)
    }

    fn total_stages(&self) -> u8 {
        match self.current_mode() {
            mode::AD => 2,
            mode::AR => 3,
            mode::ADSR => 4,
            _ => 2,
        }
    }

    /// True when the current stage is the hold/sustain stage.
    fn is_hold_stage(&self) -> bool {
        matches!(
            (self.current_mode(), self.stage),
            (mode::AR, 1) | (mode::ADSR, 2)
        )
    }

    /// Sustain level as a 0.0-1.0 multiplier.
    fn sustain_mult(&self) -> f32 {
        self.config.sustain.load(Relaxed) as f32 / 127.0
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
        if self.is_hold_stage() {
            // Hold for as long as any note is pressed; advance immediately when all released.
            return if held_notes::any_held() { u32::MAX } else { 0 };
        }

        let m = self.current_mode();
        let s = self.stage;
        let cc_val = match (m, s) {
            (_, 0) => self.config.attack.load(Relaxed),          // Attack (all modes)
            (mode::AD, 1) => self.config.decay.load(Relaxed),   // AD: Decay
            (mode::AR, 2) => self.config.release.load(Relaxed), // AR: Release
            (mode::ADSR, 1) => self.config.decay.load(Relaxed), // ADSR: Decay
            (mode::ADSR, 3) => self.config.release.load(Relaxed), // ADSR: Release
            _ => 0,
        };
        (cc_val as u32) * CC_TO_MS
    }

    fn next_stage(&mut self) -> bool {
        self.stage += 1;
        let done = self.stage >= self.total_stages();
        if done {
            self.stage = 0;
        }
        !done
    }

    fn animate(&mut self, progress: f32, _depth: f32, _offset: f32) -> super::Messages {
        // Hold stage: stay silent; the last Attack/Decay tick already set the right CC level.
        if self.is_hold_stage() {
            return None;
        }

        let m = self.current_mode();
        let s = self.stage;
        let sus = self.sustain_mult();

        let value: u8 = (match (m, s) {
            // Attack: 0 → 127
            (_, 0) => progress,
            // AD Decay: 127 → 0
            (mode::AD, 1) => 1.0 - progress,
            // AR Release: 127 → 0
            (mode::AR, 2) => 1.0 - progress,
            // ADSR Decay: 127 → sustain_level
            (mode::ADSR, 1) => 1.0 - progress * (1.0 - sus),
            // ADSR Release: sustain_level → 0
            (mode::ADSR, 3) => sus * (1.0 - progress),
            _ => 0.0,
        } * 127.0) as u8;

        let mut messages = Vec::<MidiMsg, 3>::new();
        let _ = messages.push(self.make_cc(value));
        Some(messages)
    }

    fn reset(&mut self) -> super::Messages {
        self.stage = 0;
        let mut messages = Vec::<MidiMsg, 3>::new();
        let _ = messages.push(self.make_cc(0));
        Some(messages)
    }
}
