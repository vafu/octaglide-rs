use crate::{app::MidiSender, core::MidiEvent};

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
        self.midi_sender.send(event.msg).await.unwrap();
        None
    }
}
