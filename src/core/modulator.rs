//! Timed modulation: how a consumer produces output over time.

use enum_dispatch::enum_dispatch;

use crate::core::OutputEvents;

use crate::core::consumers::SlotConsumer;

/// How a timed consumer starts and repeats.
///
/// The repeat bit lives here once; variants differ only in how a cycle starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activation {
    /// Starts on a note press and runs one cycle.
    NoteOnce,
    /// Starts on a note press and repeats.
    NoteLoop,
    /// Runs continuously from construction and repeats.
    FreeLoop,
}

impl Activation {
    /// Whether a cycle restarts after it completes.
    pub const fn loops(self) -> bool {
        matches!(self, Self::NoteLoop | Self::FreeLoop)
    }

    /// Whether a cycle needs a note press to start.
    pub const fn note_triggered(self) -> bool {
        matches!(self, Self::NoteOnce | Self::NoteLoop)
    }
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

    /// Stops the running cycle and returns any messages needed to release its
    /// output. Consumers that are not timed have nothing to release.
    fn reset(&mut self) -> OutputEvents {
        OutputEvents::new()
    }
}
