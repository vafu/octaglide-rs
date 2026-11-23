//! Platform-neutral runtime task bodies and shared channel storage.

mod storage;
mod core_task;
mod midi_dispatch;
mod buttons;
mod animations;

pub use animations::run_animations;
pub use buttons::run_buttons;
pub use core_task::{handle_event, run_core};
pub use midi_dispatch::run_midi_dispatch;
pub use storage::{RuntimeHandles, RuntimeStorage};
pub use crate::core::{ANIMATORS_PER_VOICE, MAX_VOICES};
