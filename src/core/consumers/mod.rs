use core::usize;

use heapless::Vec;

use crate::core::{MidiEvent, Output};

const CONSUMER_OUTPUT_SIZE: usize = 8;

type CoreOutput = Vec<Output, CONSUMER_OUTPUT_SIZE>;

pub trait Consumer {
    fn consume(&mut self, event: &MidiEvent) -> CoreOutput;
}

pub mod glider;
pub use self::glider::Glider;

