mod consumers;
mod transformers;

use log::info;
use midi_msg::MidiMsg;

use crate::core::consumers::ModTrigger;
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

    pub fn usb(msg: MidiMsg) -> Self {
        Self {
            msg,
            synthetic: false,
        }
    }
}

use crate::{
    app::{Animator, MidiSender},
    core::{
        consumers::{Consumer, Glider, Passthrough},
        transformers::{MidiTransformer, OctaveShifter},
    },
};

/// Consumer instances (concrete types to avoid dyn-compatibility issues with async)
struct Consumers {
    mod_trigger: ModTrigger,
    glider: Glider,
    passthrough: Passthrough,
}

pub struct Core {
    octave_shifter: OctaveShifter,
    consumers: Consumers,
}

#[derive(Debug)]
pub enum Input {
    Process(MidiEvent),
}

impl Core {
    pub fn new(
        midi_sender: MidiSender,
        glide_animator: Animator,
        envelope_animator: Animator,
    ) -> Self {
        Core {
            octave_shifter: OctaveShifter::new(),
            consumers: Consumers {
                mod_trigger: ModTrigger::new(envelope_animator),
                glider: Glider::new(midi_sender.clone(), glide_animator),
                passthrough: Passthrough::new(midi_sender),
            },
        }
    }

    pub async fn process(&mut self, input: Input) {
        match input {
            Input::Process(mut event) => {
                // Apply transformers only to user MIDI
                if !event.synthetic {
                    match self.octave_shifter.process(event.msg) {
                        Some(m) => event.msg = m,
                        None => return, // Transformer filtered it out
                    }
                }

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

                // Process through consumer chain: ModTrigger -> Glider -> Passthrough
                let event = self.consumers.mod_trigger.consume(event).await;
                if let Some(event) = event {
                    let event = self.consumers.glider.consume(event).await;
                    if let Some(event) = event {
                        self.consumers.passthrough.consume(event).await;
                    }
                }
            }
        }
    }
}
