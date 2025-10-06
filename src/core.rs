use alloc::{boxed::Box, vec::Vec};
use log::info;
use midi_msg::{MidiMsg, ParseError};

use crate::processor::{MidiProcessor, OctaveShifter};

pub struct Core {
    output: fn(Output),
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
}

impl Core {
    pub fn new(output: fn(Output)) -> Self {
        let mut processors: Vec<Box<dyn MidiProcessor>> = Vec::new();
        processors.push(Box::new(OctaveShifter::new()));
        Core { output, processors }
    }

    pub async fn process(&mut self, input: Input) {
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
                    (self.output)(Output::SendMidi(midi_msg));
                    (self.output)(Output::BlinkLed);
                }
            }
            Err(e) => info!("Midi: {:?}", e),
        }
    }
}
