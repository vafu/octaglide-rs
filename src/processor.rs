use log::info;
use midi_msg::MidiMsg;

pub struct Engine {
    send_midi: fn(MidiMsg),
}

impl Engine {
    pub fn new(send_midi: fn(MidiMsg)) -> Self {
        Engine { send_midi }
    }

    pub async fn on_message(&self, msg: Result<MidiMsg, midi_msg::ParseError>) {
        match msg {
            Ok(msg) => {
                info!("Received {:?}", msg);
                (self.send_midi)(msg)
            }
            Err(e) => info!("Midi: {:?}", e),
        }
    }
}
