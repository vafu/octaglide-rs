use super::HeldNotes;

impl HeldNotes {
    pub fn press(&mut self, note: u8) {
        self.words[note as usize / 32] |= 1 << (note % 32);
    }

    pub fn release(&mut self, note: u8) {
        self.words[note as usize / 32] &= !(1 << (note % 32));
    }

    pub fn is_held(&self, note: u8) -> bool {
        self.words[note as usize / 32] & (1 << (note % 32)) != 0
    }

    pub fn any_held(&self) -> bool {
        self.words.iter().any(|w| *w != 0)
    }
}
