//! How the UI groups a voice's 16 consumers.
//!
//! Core addresses slots flat (`0..SLOT_COUNT`); sections exist so the display
//! can label a slot and so gestures can move between groups.

use crate::core::SLOT_COUNT;

/// How many slots each section groups.
const SLOTS_PER_SECTION: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Sequencing,
    Voicing,
    Modulation,
    Reserved,
}

impl Section {
    pub const COUNT: u8 = 4;

    pub const ALL: [Section; 4] = [
        Section::Sequencing,
        Section::Voicing,
        Section::Modulation,
        Section::Reserved,
    ];

    pub const fn index(self) -> u8 {
        match self {
            Self::Sequencing => 0,
            Self::Voicing => 1,
            Self::Modulation => 2,
            Self::Reserved => 3,
        }
    }

    pub const fn from_index(index: u8) -> Self {
        match index {
            0 => Self::Sequencing,
            1 => Self::Voicing,
            2 => Self::Modulation,
            _ => Self::Reserved,
        }
    }

    pub const fn next(self) -> Self {
        Self::from_index((self.index() + 1) % Self::COUNT)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Sequencing => "SEQ",
            Self::Voicing => "VOI",
            Self::Modulation => "MOD",
            Self::Reserved => "RSV",
        }
    }

    /// Flat slot index for a slot within this section.
    pub const fn slot(self, index: u8) -> u8 {
        self.index() * SLOTS_PER_SECTION + index
    }

    /// The section a flat slot belongs to.
    pub const fn of(slot: u8) -> Self {
        Self::from_index(slot / SLOTS_PER_SECTION)
    }

    /// The index within its section for a flat slot.
    pub const fn index_of(slot: u8) -> u8 {
        slot % SLOTS_PER_SECTION
    }
}

const _: () = assert!(SLOT_COUNT == SLOTS_PER_SECTION * Section::COUNT);
