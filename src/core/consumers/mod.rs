use crate::core::{Input, OutputEvents};

mod dispatch;
pub(crate) mod envelope;
pub(crate) mod glide;
mod octave;

pub use self::envelope::{ModTrigger, ModTriggerControl};
pub use self::glide::{Glider, GliderControl};
pub use self::octave::{Octave, OctaveControl, OctaveMode};

pub const CONTROL_SLOTS: usize = 8;

/// Processes semantic input independently of MIDI, hardware, or display.
pub trait Consumer {
    type Control: Copy;
    type Mode: Copy;

    const CONTROL_MAP: [Option<Self::Control>; CONTROL_SLOTS];
    const MODE_COUNT: u8;

    async fn consume(&mut self, input: &Input, out: &mut OutputEvents) -> bool;
    fn set_control(&mut self, control: Self::Control, value: f32);
    fn control_value(&self, control: Self::Control) -> f32;
    fn mode(&self) -> Self::Mode;
    fn update_mode(&mut self, direction: i8);
}

/// Consumer instances.
#[derive(Debug)]
pub enum Consumers {
    /// An inert modulation lane; Voice still sends semantic events left by the chain.
    Passthrough,
    Octave(Octave),
    ModTrigger(ModTrigger),
    Glider(Glider),
}

/// Stable consumer identity used in snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsumerKind {
    Passthrough,
    Octave,
    ModTrigger,
    Glider,
}
