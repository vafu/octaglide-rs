use alloc::{boxed::Box, vec::Vec};
use log::info;
use midi_msg::{MidiMsg, ParseError};

use crate::{
    app::Dispatcher,
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
    BlinkLed,
    Slide(MidiMsg),
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
                    self.dispatcher.dispatch(Output::SendMidi(midi_msg)).await;
                    self.dispatcher.dispatch(Output::BlinkLed).await;
                }
            }
            Err(e) => info!("Midi: {:?}", e),
        }
    }
}
