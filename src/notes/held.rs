use super::{HeldNotes, PressResult, ReleaseResult};

impl HeldNotes {
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

    pub fn is_held(&self, note: u8) -> bool {
        self.bits[note as usize / 32] & (1 << (note % 32)) != 0
    }

    pub fn any_held(&self) -> bool {
        self.bits.iter().any(|w| *w != 0)
    }
}
