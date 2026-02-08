use heapless::Vec;

use crate::core::{MidiEvent, Output};

use super::{ConsumeResult, CoreOutput};

pub struct Passthrough;

impl Passthrough {
    pub fn new() -> Self {
        Self
    }
}

impl super::Consumer for Passthrough {
    fn consume(&mut self, event: MidiEvent) -> ConsumeResult {
        let mut res: CoreOutput = Vec::new();
        res.push(Output::SendMidi(event.msg)).ok();
        ConsumeResult::Consumed(res)
    }
}
