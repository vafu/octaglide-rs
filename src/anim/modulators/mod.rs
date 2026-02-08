use core::fmt::Debug;

use enum_dispatch::enum_dispatch;
use heapless::Vec;
use midi_msg::MidiMsg;

// TODO: create a macro with vararg of messages.
pub type Messages = Option<Vec<MidiMsg, 3>>;

#[enum_dispatch]
pub trait Modulation {
    fn animate(&mut self, progress: f32, depth: f32, offset: f32) -> Messages;
    fn reset(&mut self) -> Messages;
}

#[enum_dispatch(Modulation)]
#[derive(Debug)]
pub enum Modulator {
    Glide,
    Envelope,
}

mod envelope;
mod glide;

pub use envelope::Envelope;
pub use glide::Glide;
