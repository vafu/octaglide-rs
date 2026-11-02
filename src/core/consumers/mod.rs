use crate::core::{Input, OutputEvents};

mod dispatch;
mod slider_catch_up;

/// Exposes current 7-bit slider targets without transferring settings ownership.
pub trait SliderTarget {
    /// Return None to bypass pickup for this slider mapping.
    fn slider_value(&self, index: u8, alternate: bool) -> Option<u8>;
}

/// Gates slider edits until they reach or cross the wrapped consumer's value.
/// Other events and display-provider behavior delegate to the inner consumer.
#[derive(Debug)]
pub struct SliderCatchUp<C> {
    inner: C,
    previous: [Option<u8>; 4],
    picked_up: [bool; 4],
    alternate: bool,
}

/// Processes events independently of any display capability.
pub trait Consumer {
    /// Return true to consume the original input, false to pass it on unchanged.
    /// Core's UI consumer runs first. Unconsumed hardware reaches the selected
    /// voice consumer; unconsumed MIDI follows track routing. Voice forwards
    /// unconsumed MIDI automatically. Hardware is never forwarded as output.
    /// Append only newly generated or replacement MIDI to `out`; those messages
    /// reach subsequent consumers regardless of this Boolean result.
    async fn consume(&mut self, input: &Input, out: &mut OutputEvents) -> bool;
}

pub mod octave;
pub use self::octave::Octave;

pub mod glider;
pub use self::glider::Glider;

pub mod mod_trigger;
pub use self::mod_trigger::ModTrigger;

/// Consumer instances; UI-capable variants also implement the independent UiProvider contract.
#[derive(Debug)]
pub enum Consumers {
    /// An inert modulation lane; Voice still sends the MIDI left by the chain.
    Passthrough,
    Octave(Octave),
    ModTrigger(SliderCatchUp<ModTrigger>),
    Glider(Glider),
}
