mod consumers;
mod transformers;

use alloc::{boxed::Box, vec::Vec};
use log::info;
use midi_msg::{MidiMsg, ParseError};

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
    anim::animator::Cmd,
    app::{AnimatorSender, MidiSender},
    core::{
        consumers::{Consumer, Glider, Passthrough},
        transformers::{MidiTransformer, OctaveShifter},
    },
};

pub struct Core {
    midi_sender: MidiSender,
    animator_sender: AnimatorSender,
    transformers: Vec<Box<dyn MidiTransformer>>,
    consumers: Vec<Box<dyn Consumer>>,
}

#[derive(Debug)]
pub enum Input {
    Process(MidiEvent),
}

#[derive(Debug)]
pub enum Output {
    SendMidi(MidiMsg),
    Animate(Cmd),
    BlinkLed,
}

impl Core {
    pub fn new(midi_sender: MidiSender, animator_sender: AnimatorSender) -> Self {
        let transformers: Vec<Box<dyn MidiTransformer>> = vec![Box::new(OctaveShifter::new())];
        let consumers: Vec<Box<dyn Consumer>> = vec![
            Box::new(Glider::new()),
            Box::new(Passthrough::new()), // LAST: routes all unconsumed events
        ];
        Core {
            midi_sender,
            animator_sender,
            transformers,
            consumers,
        }
    }

    pub async fn process(&mut self, input: Input) {
        match input {
            Input::Process(mut event) => {
                // Handle parse errors

                // Apply transformers only to user MIDI
                let transformed_msg = if !event.synthetic {
                    match self
                        .transformers
                        .iter_mut()
                        .try_fold(event.msg, |m, p| p.process(m))
                    {
                        Some(m) => m,
                        None => return, // Transformer filtered it out
                    }
                } else {
                    event.msg
                };

                event.msg = transformed_msg;

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

                let mut event = event;
                // Process the event through consumers (serial chain with early exit)
                for c in self.consumers.iter_mut() {
                    use consumers::ConsumeResult;

                    match c.consume(event) {
                        ConsumeResult::Consumed(outputs) => {
                            // Process outputs and stop consuming chain
                            for out in outputs {
                                match out {
                                    Output::SendMidi(msg) => {
                                        self.midi_sender.send(msg.clone()).await.unwrap();
                                    }
                                    Output::BlinkLed => {
                                        crate::app::blink_led::spawn().ok();
                                    }
                                    Output::Animate(cmd) => {
                                        self.animator_sender.send(cmd).await.unwrap();
                                    }
                                }
                            }
                            break; // Don't pass to other consumers
                        }
                        ConsumeResult::Ignored(ev) => {
                            // Continue to next consumer
                            event = ev;
                            continue;
                        }
                    }
                }
            }
        }
    }
}
