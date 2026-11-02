mod buttons;
#[cfg(target_os = "none")]
mod display;
#[cfg(any(test, target_os = "none"))]
mod encoder;
#[cfg(any(test, target_os = "none"))]
mod envelope_plot;
#[cfg(target_os = "none")]
mod hardware;
mod navigation;

#[cfg(any(test, target_os = "none"))]
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

/// Emits one press until the button has been released for a debounce interval.
#[derive(Debug)]
pub struct ButtonDebouncer {
    latched: bool,
    released_at_ms: Option<u32>,
}

/// Core-level consumer that handles navigation before events reach voices.
/// Hold the encoder switch, press button 1-4, then release that button to select
/// its track. Releasing the encoder first cancels the pending selection.
/// Without the encoder modifier, a button click selects its modulation lane.
#[derive(Debug)]
pub struct UiConsumer {
    encoder_held: bool,
    captured_buttons: u8,
    armed_buttons: u8,
    lane_buttons: u8,
    pending_track: Option<crate::core::TrackId>,
    pending_lane: Option<usize>,
    sliders: [Option<u16>; 4],
}

/// Raw GPIO transition before debounce: indices 0-3 are buttons, 4 is the encoder switch.
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

/// Supplies display data and tracks pending changes independently of event consumption.
pub trait UiProvider {
    /// Owned display data produced by this provider.
    type Model;

    /// Read the latest model without acknowledging pending changes.
    /// Page switches use this even when no update is pending.
    fn ui_snapshot(&self) -> Self::Model;

    /// Return whether displayed values changed, then clear the pending flag.
    /// Multiple changes coalesce; subsequent calls return false until another change.
    /// After publishing on a page switch, call this to acknowledge pending changes.
    fn has_updates(&mut self) -> bool;
}

/// Latest-only transport of display snapshots. Superseded updates coalesce.
pub type UiWriter = rtic_sync::signal::SignalWriter<'static, DisplaySnapshot>;
#[cfg(target_os = "none")]
pub type UiReader = rtic_sync::signal::SignalReader<'static, DisplaySnapshot>;

/// Display data owned by an octave consumer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OctaveUi {
    pub shift: i8,
}

/// Display data owned by an envelope-trigger consumer.
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

/// Display data owned by a glide consumer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GliderUi {
    pub enabled: bool,
    pub duration_ms: u32,
    pub bend_range_semitones: f32,
    pub note_on_velocity: u8,
}

/// Display models supplied by UI providers. Additional page models can be added here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiModel {
    Octave(OctaveUi),
    ModTrigger(ModTriggerUi),
    Glider(GliderUi),
}

/// Display values for one envelope ramp, independent of consumer settings types.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnvelopeStageUi {
    pub duration: u8,
    pub curve: u8,
}

/// Display selection and page model, independent of Core's routing types.
/// Track, voice and lane indices are zero-based; the renderer shows 1 through 4.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplaySnapshot {
    pub track: u8,
    pub voice: u8,
    /// Selected modulation lane, zero-based; buttons/display use 1 through 4.
    pub lane: u8,
    /// A USB MIDI device is connected and ready on the host port.
    pub usb_connected: bool,
    pub model: Option<UiModel>,
}
