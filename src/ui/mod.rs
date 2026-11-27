mod buttons;
mod controller;
mod display;
mod encoder;
mod envelope_plot;
mod snapshot;

pub use buttons::{take_next_release, ButtonDebouncer};
pub use controller::{Controller, UiEvent};
pub(crate) use envelope_plot::envelope_plot_level;
pub use display::draw_snapshot;
pub use encoder::EncoderDecoder;

#[cfg(test)]
mod test;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonEdge {
    pub index: u8,
    pub pressed: bool,
    pub at_ms: u32,
}

pub const ENCODER_BUTTON_INDEX: u8 = 4;
pub const BUTTON_QUEUE_CAPACITY: usize = 16;
pub type ButtonSender = rtic_sync::channel::Sender<'static, ButtonEdge, BUTTON_QUEUE_CAPACITY>;
pub type ButtonReceiver = rtic_sync::channel::Receiver<'static, ButtonEdge, BUTTON_QUEUE_CAPACITY>;

pub type UiWriter = rtic_sync::signal::SignalWriter<'static, DisplaySnapshot>;
pub type UiReader = rtic_sync::signal::SignalReader<'static, DisplaySnapshot>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModTriggerUi {
    pub name: &'static str,
    pub output_cc: u8,
    pub attack: EnvelopeStageUi,
    pub decay: EnvelopeStageUi,
    pub sustain: u8,
    pub release: EnvelopeStageUi,
    pub mode: u8,
}

/// The consumer model the display renders.
///
/// One variant while only modulation lanes are addressable. Voicing consumers
/// will arrive as their own model once `Core` can address them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiModel {
    ModTrigger(ModTriggerUi),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnvelopeStageUi {
    pub duration: u8,
    pub curve: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplaySnapshot {
    pub track: u8,
    pub voice: u8,
    pub lane: u8,
    pub usb_connected: bool,
    pub model: Option<UiModel>,
}
