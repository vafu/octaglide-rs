use heapless::Vec;
use log::info;
use midi_msg::MidiMsg;

use crate::processor::{MidiProcessor, MidiProcessors, OctaveShifter};

const PROCESSOR_COUNT: usize = 2;

pub struct Engine {
    send_midi: fn(MidiMsg),
    processors: Vec<MidiProcessors, PROCESSOR_COUNT>,
}

impl Engine {
    pub fn new(send_midi: fn(MidiMsg)) -> Self {
        let mut processors = Vec::new();
        processors
            .push(MidiProcessors::OctaveShifter(OctaveShifter::new()))
            .ok();
        Engine {
            send_midi,
            processors,
        }
    }

    pub async fn on_message(&mut self, msg: Result<MidiMsg, midi_msg::ParseError>) {
        match msg {
            Ok(msg) => {
                info!("Received {:?}", msg);

                let final_msg = self
                    .processors
                    .iter_mut()
                    .try_fold(msg, |m, p| p.process(m));

                if let Some(midi_msg) = final_msg {
                    (self.send_midi)(midi_msg)
                }
            }
            Err(e) => info!("Midi: {:?}", e),
        }
    }
}
