//! Animation runtime for Core-owned modulators.

#[cfg(target_os = "none")]
mod runtime;
#[cfg(target_os = "none")]
pub use runtime::run_animations;
