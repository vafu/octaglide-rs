//! Platform-neutral runtime task bodies and shared channel storage.

mod channels;
mod core_task;
mod midi_dispatch;
mod buttons;
mod animations;

pub use animations::run_animations;
pub use buttons::run_buttons;
#[cfg(any(feature = "host", test))]
pub use core_task::handle_event;
pub use core_task::run_core;
pub use midi_dispatch::run_midi_dispatch;
pub use channels::RuntimeStorage;
pub use crate::core::{ANIMATORS_PER_VOICE, MAX_VOICES};
#[cfg(any(feature = "host", test))]
pub use channels::RuntimeHandles;
