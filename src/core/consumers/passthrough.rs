use heapless::Vec;
use log::info;
use midi_msg::MidiMsg;

use crate::core::{MidiEvent, Output};
use crate::midi_fmt::MidiFmt;

use super::{ConsumeResult, CoreOutput};

/// Passthrough consumer - routes all unconsumed MIDI to the output bus
///
/// This consumer should be placed LAST in the consumer chain.
/// It forwards any messages that earlier consumers didn't handle.
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
