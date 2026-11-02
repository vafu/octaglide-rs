use super::{HeldNotes, NOTES};
use crate::core::VoiceId;

/// Read one voice's held notes, releasing access before returning.
pub async fn read_notes<R>(voice: VoiceId, f: impl FnOnce(&HeldNotes) -> R) -> Option<R> {
    let notes = NOTES.get(voice.0 as usize)?.access().await;
    Some(f(&notes))
}

/// Update one voice's held notes, releasing access before returning.
pub async fn edit_notes<R>(voice: VoiceId, f: impl FnOnce(&mut HeldNotes) -> R) -> Option<R> {
    let mut notes = NOTES.get(voice.0 as usize)?.access().await;
    Some(f(&mut notes))
}
