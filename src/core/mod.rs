mod consumers;
mod transformers;

use alloc::vec;
use log::info;
use midi_msg::MidiMsg;

use crate::core::{consumers::ModTrigger, transformers::Transformers};
use crate::midi_fmt::MidiFmt;

/// MIDI message with metadata about its origin and context
#[derive(Debug, Clone)]
pub struct MidiEvent {
    pub msg: MidiMsg,
    pub synthetic: bool,
}

impl MidiEvent {
    pub fn from_user(msg: MidiMsg) -> Self {
        Self {
            msg,
            synthetic: false,
        }
    }

    pub fn synthetic(msg: MidiMsg) -> Self {
        Self {
            msg,
            synthetic: true,
        }
    }
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
                // Apply transformers only to user MIDI
                if !event.synthetic {
                    for transformer in &mut self.transformers {
                        let processed = transformer.process(event.msg);
                        let Some(m) = processed else { return };
                        event.msg = m;
                    }
                }

                log_event(&event);

                // Process through consumer chain
                let mut event = Some(event);
                for consumer in &mut self.consumers {
                    let Some(e) = event else { break };
                    event = consumer.consume(e).await;
                }
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
        let synthetic = if event.synthetic { " [synthetic]" } else { "" };
        info!("<<< {}{}", MidiFmt(&event.msg), synthetic);
    }
}
