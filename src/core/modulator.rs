//! Timed modulation: how a consumer produces output over time.

use enum_dispatch::enum_dispatch;

use crate::core::OutputEvents;

use crate::core::consumers::SlotConsumer;

/// How a timed consumer starts and repeats.
///
/// Each variant carries the two facts that define it, so there is no separate
/// `impl` restating the same bit twice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// Starts on a note press and runs one cycle.
    NoteOnce { note_trigger: bool, looping: bool },
    /// Starts on a note press and repeats.
    NoteLoop { note_trigger: bool, looping: bool },
    /// Runs continuously from construction and repeats.
    FreeLoop { note_trigger: bool, looping: bool },
}

impl Trigger {
    pub const NOTE_ONCE: Self = Self::NoteOnce { note_trigger: true, looping: false };
    pub const NOTE_LOOP: Self = Self::NoteLoop { note_trigger: true, looping: true };
    pub const FREE_LOOP: Self = Self::FreeLoop { note_trigger: false, looping: true };

    /// Whether a cycle restarts after it completes.
    pub const fn loops(self) -> bool {
        match self {
            Self::NoteOnce { looping, .. }
            | Self::NoteLoop { looping, .. }
            | Self::FreeLoop { looping, .. } => looping,
        }
    }

    /// Whether a cycle needs a note press to start.
    pub const fn note_triggered(self) -> bool {
        match self {
            Self::NoteOnce { note_trigger, .. }
            | Self::NoteLoop { note_trigger, .. }
            | Self::FreeLoop { note_trigger, .. } => note_trigger,
        }
    }

    pub const fn index(self) -> u8 {
        match self {
            Self::NoteOnce { .. } => 0,
            Self::NoteLoop { .. } => 1,
            Self::FreeLoop { .. } => 2,
        }
    }

    pub const fn from_index(index: u8) -> Self {
        match index {
            1 => Self::NOTE_LOOP,
            2 => Self::FREE_LOOP,
            _ => Self::NOTE_ONCE,
        }
    }

    pub const COUNT: u8 = 3;
}

/// A consumer that produces timed output.
///
/// `advance` is called once per tick with the elapsed milliseconds and whether
/// a note is currently held. Returning `None` means the consumer produced
/// nothing this tick; that is also how an idle consumer reports itself.
#[enum_dispatch]
pub trait Modulation {
    /// Advances this consumer by `delta_ms`.
    ///
    /// Consumers that are not timed keep the default, which produces nothing.
    fn advance(&mut self, _delta_ms: u32, _gated: bool) -> Option<OutputEvents> {
        None
    }

}
