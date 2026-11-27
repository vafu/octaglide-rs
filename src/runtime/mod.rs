//! Platform-neutral runtime task bodies and shared channel storage.

mod buttons;
mod channels;
mod core_task;
mod midi_dispatch;

pub use buttons::run_buttons;
#[cfg(any(feature = "host", test))]
pub use channels::RuntimeHandles;
pub use channels::RuntimeStorage;
#[cfg(any(feature = "host", test))]
pub use core_task::handle_event;
pub use core_task::run_core;
pub use midi_dispatch::run_midi_dispatch;
