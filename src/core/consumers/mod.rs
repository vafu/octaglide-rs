use crate::core::MidiEvent;

pub trait Consumer {
    /// Consume a MIDI event
    ///
    /// Returns:
    /// - `Some(event)` - event ignored, pass to next consumer
    /// - `None` - event consumed, stop chain
    async fn consume(&mut self, event: MidiEvent) -> Option<MidiEvent>;
}

pub mod glider;
pub use self::glider::Glider;

pub mod passthrough;
pub use self::passthrough::Passthrough;

pub mod mod_trigger;
pub use self::mod_trigger::ModTrigger;
