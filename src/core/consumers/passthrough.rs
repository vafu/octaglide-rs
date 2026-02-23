use crate::{app::MidiSender, core::{MidiEvent, MidiOut}};

#[derive(Debug)]
pub struct Passthrough {
    midi_sender: MidiSender,
}

impl Passthrough {
    pub fn new(midi_sender: MidiSender) -> Self {
        Self { midi_sender }
    }
}

impl super::Consumer for Passthrough {
    async fn consume(&mut self, event: MidiEvent) -> Option<MidiEvent> {
        self.midi_sender.send(MidiOut { msg: event.msg, tag: "pass" }).await.unwrap();
        None
    }
}
