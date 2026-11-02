//! Shared held-note tracking, indexed by voice; no parameter or routing state.

mod access;
mod held;

use crate::core::MAX_VOICES;
use heapless::Vec;
use rtic_sync::arbiter::Arbiter;

pub use access::{edit_notes, read_notes};

pub const MAX_HELD_NOTES: usize = 8;

#[derive(Debug)]
pub struct HeldNotes {
    pub notes: Vec<u8, MAX_HELD_NOTES>,
    pub bits: [u32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PressResult {
    pub previous_top: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReleaseResult {
    pub was_top: bool,
    pub new_top: Option<u8>,
}

static NOTES: [Arbiter<HeldNotes>; MAX_VOICES] = [const {
    Arbiter::new(HeldNotes {
        notes: Vec::new(),
        bits: [0; 4],
    })
}; MAX_VOICES];
