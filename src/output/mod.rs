//! Messages leaving the semantic voice layer.

use midi_msg::MidiMsg;
use rtic_sync::channel::{Receiver, Sender};

use crate::core::VoiceOutput;

pub const OUTPUT_CHANNEL_CAPACITY: usize = 32;

#[derive(Clone, Debug, PartialEq)]
pub enum OutputMessage {
    Semantic(VoiceOutput),
    Raw(MidiMsg),
}

pub type OutputSender = Sender<'static, OutputMessage, OUTPUT_CHANNEL_CAPACITY>;
pub type OutputReceiver = Receiver<'static, OutputMessage, OUTPUT_CHANNEL_CAPACITY>;
