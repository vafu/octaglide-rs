mod buttons;
mod controller;
#[cfg(target_os = "none")]
mod display;
#[cfg(any(test, target_os = "none"))]
mod encoder;
#[cfg(any(test, target_os = "none"))]
mod envelope_plot;
#[cfg(target_os = "none")]
mod hardware;
mod snapshot;

pub use controller::{Controller, UiEvent};
pub(crate) use envelope_plot::envelope_plot_level;
#[cfg(target_os = "none")]
pub use display::run_envelope_display;
#[cfg(target_os = "none")]
pub use encoder::EncoderDecoder;
#[cfg(target_os = "none")]
pub use hardware::{
    handle_buttons_gpio3_interrupt, handle_buttons_gpio4_interrupt, handle_encoder_interrupt,
    read_sliders, run_buttons,
};

#[cfg(test)]
mod test;

#[derive(Debug)]
pub struct ButtonDebouncer {
    latched: bool,
    released_at_ms: Option<u32>,
}

#[cfg(target_os = "none")]
pub struct ButtonEdge {
    pub index: u8,
    pub pressed: bool,
    pub at_ms: u32,
}

#[cfg(target_os = "none")]
pub const ENCODER_BUTTON_INDEX: u8 = 4;
#[cfg(target_os = "none")]
pub const BUTTON_QUEUE_CAPACITY: usize = 16;
#[cfg(target_os = "none")]
pub type ButtonSender = rtic_sync::channel::Sender<'static, ButtonEdge, BUTTON_QUEUE_CAPACITY>;
#[cfg(target_os = "none")]
pub type ButtonReceiver = rtic_sync::channel::Receiver<'static, ButtonEdge, BUTTON_QUEUE_CAPACITY>;

pub type UiWriter = rtic_sync::signal::SignalWriter<'static, DisplaySnapshot>;
#[cfg(target_os = "none")]
pub type UiReader = rtic_sync::signal::SignalReader<'static, DisplaySnapshot>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OctaveUi {
    pub shift: i8,
}

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GliderUi {
    pub enabled: bool,
    pub duration_ms: u32,
    pub bend_range_semitones: f32,
    pub note_on_velocity: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiModel {
    Octave(OctaveUi),
    ModTrigger(ModTriggerUi),
    Glider(GliderUi),
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
