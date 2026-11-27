//! Octave settings owned by the octave consumer.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OctaveState {
    pub shift: i8,
}

impl OctaveState {
    pub const INITIAL: Self = Self { shift: 0 };
}
