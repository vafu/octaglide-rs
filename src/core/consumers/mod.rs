use core::usize;

use heapless::Vec;

use crate::core::{MidiEvent, Output};

const CONSUMER_OUTPUT_SIZE: usize = 8;

type CoreOutput = Vec<Output, CONSUMER_OUTPUT_SIZE>;

/// Result of consuming a MIDI event
pub enum ConsumeResult {
    /// Event was consumed - do not pass to other consumers
    Consumed(CoreOutput),
    /// Event was ignored - ownership returned to caller for next consumer
    /// Optionally produces outputs (e.g., triggering animations) while passing event along
    Ignored(MidiEvent, CoreOutput),
}

impl ConsumeResult {
    /// Create an Ignored result with no outputs (most common case)
    pub fn ignored(event: MidiEvent) -> Self {
        ConsumeResult::Ignored(event, Vec::new())
    }
}

pub trait Consumer {
    /// Consume a MIDI event and optionally produce outputs
    ///
    /// Takes ownership of the event. If the event is not handled:
    /// - Return `Ignored(event)` to pass ownership to the next consumer
    /// - Return `Consumed(outputs)` to consume the event and produce outputs
    ///
    /// This avoids cloning - ownership is transferred through the chain.
    fn consume(&mut self, event: MidiEvent) -> ConsumeResult;
}

pub mod glider;
pub use self::glider::Glider;

pub mod passthrough;
pub use self::passthrough::Passthrough;

pub mod mod_trigger;
pub use self::mod_trigger::ModTrigger;
