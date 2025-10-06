use log::info;
use midi_msg::MidiMsg;

use crate::processor::{MidiProcessor, OctaveShifter};

pub struct Engine {
    send_midi: fn(MidiMsg),
    shifter: OctaveShifter,
}

impl Engine {
    pub fn new(send_midi: fn(MidiMsg)) -> Self {
        Engine {
            send_midi,
            shifter: OctaveShifter::new(),
        }
    }

    pub async fn on_message(&mut self, msg: Result<MidiMsg, midi_msg::ParseError>) {
        match msg {
            Ok(msg) => {
                info!("Received {:?}", msg);
                match self.shifter.process(msg) {
                    Some(message) => (self.send_midi)(message),
                    None => info!("Consumed"),
                }
            }
            Err(e) => info!("Midi: {:?}", e),
        }
    }
}
