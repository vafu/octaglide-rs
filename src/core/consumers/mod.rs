use crate::core::event::{Event, Events};
use enum_dispatch::enum_dispatch;

#[enum_dispatch]
pub trait Consumer {
    /// Consume, transform, or fan out an event.
    async fn consume(&mut self, event: Event, out: &mut Events);
}

pub mod octave;
pub use self::octave::Octave;

pub mod glider;
pub use self::glider::Glider;

pub mod passthrough;
pub use self::passthrough::Passthrough;

pub mod mod_trigger;
pub use self::mod_trigger::ModTrigger;

/// Enum wrapper for all consumer types
#[enum_dispatch(Consumer)]
#[derive(Debug)]
pub enum Consumers {
    Octave,
    ModTrigger,
    Glider,
    Passthrough,
}
