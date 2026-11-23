mod buttons;
mod controller;
mod display;
mod encoder;
mod envelope_plot;
#[cfg(target_os = "none")]
mod hardware;
mod snapshot;

pub use buttons::{take_next_release, ButtonDebouncer};
pub use controller::{Controller, UiEvent};
pub(crate) use envelope_plot::envelope_plot_level;
pub use display::draw_snapshot;
#[cfg(target_os = "none")]
pub use display::run_envelope_display;
pub use encoder::EncoderDecoder;
#[cfg(target_os = "none")]
pub use hardware::{
    handle_buttons_gpio3_interrupt, handle_buttons_gpio4_interrupt, handle_encoder_interrupt,
    read_sliders,
};

#[cfg(test)]
mod test;

#[derive(Debug)]
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
