use core::fmt::Debug;

use enum_dispatch::enum_dispatch;
use heapless::Vec;
use midi_msg::MidiMsg;

mod curve;

/// Shared envelope shape calculation for playback and display previews.
pub use curve::envelope_level;

// TODO(octaglide-task-31): create a macro with vararg of messages.
pub type Messages = Option<Vec<MidiMsg, 3>>;

#[enum_dispatch]
pub trait Modulation {
    /// Apply new parameters while retaining animation progress.
    fn update(&mut self, settings: &super::Settings);
    async fn duration_ms(&self) -> u32;
    async fn animate(&mut self, progress: f32, depth: f32, offset: f32) -> Messages;
    async fn reset(&mut self) -> Messages;
    async fn next_stage(&mut self) -> bool;
}

#[enum_dispatch(Modulation)]
#[derive(Debug)]
pub enum Modulator {
    Glide,
    Envelope,
}

pub mod envelope;
pub mod glide;

pub use envelope::Envelope;
pub use glide::Glide;
