use alloc::{boxed::Box, vec::Vec};
use log::info;
use midi_msg::{ChannelVoiceMsg, ControlChange, MidiMsg, ParseError};

use crate::{
    app::Dispatcher,
    engine::EngineMessage,
    processor::{MidiProcessor, OctaveShifter},
};

pub struct Core {
    dispatcher: Dispatcher,
    processors: Vec<Box<dyn MidiProcessor>>,
}

#[derive(Debug)]
pub enum Input {
    ProcessMidi(Result<MidiMsg, ParseError>),
}

#[derive(Debug)]
pub enum Output {
    SendMidi(MidiMsg),
    Engine(EngineMessage),
    BlinkLed,
}

impl Core {
    pub fn new(dispatcher: Dispatcher) -> Self {
        let processors: Vec<Box<dyn MidiProcessor>> = vec![Box::new(OctaveShifter::new())];
        Core {
            dispatcher,
            processors,
        }
    }

    pub async fn process(&mut self, input: Input) {
        let a: Option<u8> = Some(5);
        let b: Option<u8> = None;

        if let Some(b) = a {}

        match input {
            Input::ProcessMidi(midi_msg) => self.process_midi(midi_msg).await,
        }
    }

    async fn process_midi(&mut self, msg: Result<MidiMsg, midi_msg::ParseError>) {
        match msg {
            Ok(msg) => {
                info!("Received {:?}", msg);

                let final_msg = self
                    .processors
                    .iter_mut()
                    .try_fold(msg, |m, p| p.process(m));

                if let Some(midi_msg) = final_msg {
                    let start_glide = matches!(
                        &midi_msg,
                        MidiMsg::ChannelVoice {
                            msg: ChannelVoiceMsg::NoteOn { .. },
                            ..
                        }
                    );
                    let stop_glide = matches!(
                        &midi_msg,
                        MidiMsg::ChannelVoice {
                            msg: ChannelVoiceMsg::NoteOff { .. },
                            ..
                        }
                    );
                    if let MidiMsg::ChannelVoice {
                        msg:
                            ChannelVoiceMsg::ControlChange {
                                control: ControlChange::CC { control: 13, value },
                            },
                        ..
                    } = midi_msg
                    {
                        self.dispatcher
                            .dispatch(Output::Engine(EngineMessage::Duration(
                                value as u32 * 1000 / 127,
                            )))
                            .await;
                    }
                    if let MidiMsg::ChannelVoice {
                        msg:
                            ChannelVoiceMsg::ControlChange {
                                control: ControlChange::CC { control: 14, value },
                            },
                        ..
                    } = midi_msg
                    {
                        self.dispatcher
                            .dispatch(Output::Engine(EngineMessage::Looping(value > 127 / 2)))
                            .await;
                    }
                    self.dispatcher.dispatch(Output::SendMidi(midi_msg)).await;
                    if start_glide {
                        self.dispatcher
                            .dispatch(Output::Engine(EngineMessage::Start))
                            .await;
                    }
                    if stop_glide {
                        self.dispatcher
                            .dispatch(Output::Engine(EngineMessage::Stop))
                            .await;
                    }
                    self.dispatcher.dispatch(Output::BlinkLed).await;
                }
            }
            Err(e) => info!("Midi: {:?}", e),
        }
    }
}
