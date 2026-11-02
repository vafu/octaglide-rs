//! Animation commands carry copies of consumer-owned settings.

#[cfg(target_os = "none")]
pub mod animator;
pub mod modulators;
#[cfg(target_os = "none")]
mod runtime;
#[cfg(target_os = "none")]
pub use runtime::run_animations;

/// One glide engine and two envelope engines per voice.
pub const ANIMATORS_PER_VOICE: usize = 3;

/// Independent animation command handles required to construct one voice.
#[derive(Debug)]
pub struct VoiceAnimators {
    pub glide: crate::app::Animator,
    pub envelopes: [crate::app::Animator; 2],
}

use crate::core::{EnvelopeState, GlideState};
use modulators::Modulator;

/// Commands sent from a consumer to its animation engine.
#[derive(Debug)]
pub enum Cmd {
    Start(Modulator),
    /// Apply live settings without replacing or restarting the running animation.
    Update(Settings),
    Stop,
}

/// Settings snapshots from the consumer that owns an animation.
#[derive(Clone, Copy, Debug)]
pub enum Settings {
    Envelope(EnvelopeState),
    Glide(GlideState),
}
