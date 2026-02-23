use core::sync::atomic::{AtomicU8, Ordering::Relaxed};

use heapless::Vec;
use log::info;
use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::anim::modulators::Modulation;

const CC_TO_MS: u32 = 16;

#[derive(Debug)]
pub struct EnvelopeConfig {
    pub attack: AtomicU8,
    pub release: AtomicU8,
}

impl EnvelopeConfig {
    const fn default() -> Self {
        Self {
            attack: AtomicU8::new(20),
            release: AtomicU8::new(20),
        }
    }

    pub fn total_ms(&self) -> u32 {
        let a = self.attack.load(Relaxed) as u32;
        let r = self.release.load(Relaxed) as u32;
        ((a + r) * CC_TO_MS).max(1)
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

const STAGES: u8 = 2;

impl Envelope {
    pub fn new(ch: Channel, cc: u8, config: &'static EnvelopeConfig) -> Self {
        Self {
            ch,
            cc,
            config,
            stage: 0,
        }
    }
}

impl Modulation for Envelope {
    fn duration_ms(&self) -> u32 {
        (match self.stage {
            0 => self.config.attack.load(Relaxed),
            1 => self.config.release.load(Relaxed),
            _ => 0,
        } as u32)
            * CC_TO_MS
    }

    fn next_stage(&mut self) -> bool {
        self.stage += 1;
        let done = self.stage == STAGES;
        if done {
            info!("done anim");
            self.stage = 0;
        }
        !done
    }

    fn animate(&mut self, progress: f32, _depth: f32, _offset: f32) -> super::Messages {
        let mult = if self.stage == 1 {
            1.0 - progress
        } else {
            progress
        };
        info!("mult {}", mult);
        let value: u8 = (mult * 127.0) as u8;
        let mut messages = Vec::<MidiMsg, 3>::new();
        let _ = messages.push(MidiMsg::ChannelVoice {
            channel: self.ch,
            msg: ChannelVoiceMsg::ControlChange {
                control: ControlChange::CC {
                    control: self.cc,
                    value,
                },
            },
        });

        Some(messages)
    }

    fn reset(&mut self) -> super::Messages {
        let mut messages = Vec::<MidiMsg, 3>::new();

        let _ = messages.push(MidiMsg::ChannelVoice {
            channel: self.ch,
            msg: ChannelVoiceMsg::ControlChange {
                control: ControlChange::CC {
                    control: self.cc,
                    value: 0,
                },
            },
        });

        Some(messages)
    }
}
