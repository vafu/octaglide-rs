//! Held-note tracking, owned by Core.
//!
//! Core updates this stack once per incoming note and derives the gate and
//! the press/release transitions that consumers receive.

use heapless::Vec;

use super::VoiceId;

pub const MAX_HELD_NOTES: usize = 8;

#[derive(Debug)]
pub struct HeldNotes {
    notes: Vec<u8, MAX_HELD_NOTES>,
    bits: [u32; 4],
}

impl HeldNotes {
    pub const fn new() -> Self {
        Self { notes: Vec::new(), bits: [0; 4] }
    }

    pub fn press(&mut self, note: u8) -> PressResult {
        if let Some(pos) = self.notes.iter().position(|&held| held == note) {
            self.notes.remove(pos);
        }
        let previous_top = self.notes.last().copied();
        self.notes.push(note).ok();
        self.bits[note as usize / 32] |= 1 << (note % 32);
        PressResult { previous_top }
    }

    pub fn release(&mut self, note: u8) -> Option<ReleaseResult> {
        let pos = self.notes.iter().position(|&held| held == note)?;
        let was_top = pos == self.notes.len() - 1;
        self.notes.remove(pos);
        self.bits[note as usize / 32] &= !(1 << (note % 32));
        Some(ReleaseResult {
            was_top,
            new_top: self.notes.last().copied(),
        })
    }

    pub fn any_held(&self) -> bool {
        self.bits.iter().any(|w| *w != 0)
    }
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

/// Which note edge this transition describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteKind {
    Press,
    Release,
}

/// A note edge with the stack transitions Core already computed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoteTransition {
    pub voice: VoiceId,
    pub kind: NoteKind,
    pub note: u8,
    pub velocity: u8,
    /// Held note below this one before the press, if any.
    pub previous_top: Option<u8>,
    /// For a release: whether the released note was the top of the stack.
    pub was_top: bool,
    /// For a release: the new top of the stack, if any.
    pub new_top: Option<u8>,
    /// Whether any note remains held after this edge.
    pub any_held: bool,
}
