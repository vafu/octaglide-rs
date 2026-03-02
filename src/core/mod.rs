mod consumers;
mod transformers;

use alloc::vec;
use core::sync::atomic::Ordering::Relaxed;
use log::info;
use midi_msg::MidiMsg;

use crate::anim::modulators::envelope::CONFIGS;
use crate::core::{consumers::ModTrigger, transformers::Transformers};
use crate::midi_fmt::MidiFmt;

#[derive(Debug, Clone)]
pub struct MidiEvent {
    pub msg: MidiMsg,
}

impl MidiEvent {
    pub fn from_user(msg: MidiMsg) -> Self {
        Self { msg }
    }
}

#[derive(Debug)]
pub struct MidiOut {
    pub msg: MidiMsg,
    pub tag: &'static str,
}

use crate::{
    app::{Animator, MidiSender},
    core::{
        consumers::{Consumer, Consumers, Glider, Passthrough},
        transformers::{MidiTransformer, OctaveShifter},
    },
};
use heapless::Vec;

pub struct Core {
    transformers: Vec<Transformers, 8>,
    consumers: Vec<Consumers, 8>,
}

#[derive(Debug)]
pub enum Input {
    Process(MidiEvent),
    AnalogUpdate { index: u8, value: u16 },
    /// Encoder rotated: +1 = CW, -1 = CCW.
    /// Generic — currently cycles envelope mode, will drive menus/params later.
    EncoderStep(i8),
    /// Encoder push-button clicked.
    EncoderClick,
}

macro_rules! vec {
    ($($item:expr),* $(,)?) => {{
        let mut v = Vec::new();
        $(v.push($item).unwrap();)*
        v
    }};
}

impl Core {
    pub fn new(
        midi_sender: MidiSender,
        glide_animator: Animator,
        envelope_animator: Animator,
    ) -> Self {
        let transformers = vec![Transformers::OctaveShifter(OctaveShifter::new())];
        let consumers = vec![
            Consumers::ModTrigger(ModTrigger::new(envelope_animator)),
            Consumers::Glider(Glider::new(midi_sender.clone(), glide_animator)),
            Consumers::Passthrough(Passthrough::new(midi_sender)),
        ];

        Core {
            transformers,
            consumers,
        }
    }

    pub async fn process(&mut self, input: Input) {
        match input {
            Input::Process(mut event) => {
                for transformer in &mut self.transformers {
                    let processed = transformer.process(event.msg);
                    let Some(m) = processed else { return };
                    event.msg = m;
                }

                log_event(&event);

                // Process through consumer chain
                let mut event = Some(event);
                for consumer in &mut self.consumers {
                    let Some(e) = event else { break };
                    event = consumer.consume(e).await;
                }
            }
            Input::AnalogUpdate { index, value } => {
                // Map 10-bit ADC (0-1023) to 0-127
                let param = (value >> 3) as u8;
                let config = &CONFIGS[0];
                match index {
                    3 => config.attack.store(param, Relaxed),
                    2 => config.decay.store(param, Relaxed),
                    1 => config.sustain.store(param, Relaxed),
                    0 => config.release.store(param, Relaxed),
                    _ => {}
                }
            }
            Input::EncoderStep(delta) => {
                // Cycle envelope mode: AD(0) → AR(1) → ADSR(2) → wrap
                let config = &CONFIGS[0];
                let current = config.mode.load(Relaxed) as i8;
                let next = (current + delta).rem_euclid(3) as u8;
                config.mode.store(next, Relaxed);
                info!("Envelope mode → {}", next);
            }
            Input::EncoderClick => {
                // TODO: implement encoder click action
            }
        }
    }
}

fn log_event(event: &MidiEvent) {
    // Format and log event
    // Skip logging PitchBend to avoid noise
    if !matches!(
        event.msg,
        MidiMsg::ChannelVoice {
            msg: midi_msg::ChannelVoiceMsg::PitchBend { .. },
            ..
        }
    ) {
        info!("<<< {}", MidiFmt(&event.msg));
    }
}
