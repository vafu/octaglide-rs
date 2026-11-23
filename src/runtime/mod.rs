//! Platform-neutral runtime task bodies and shared channel storage.

mod storage;

pub use storage::{RuntimeHandles, RuntimeStorage};
pub use crate::core::{ANIMATORS_PER_VOICE, MAX_VOICES};
