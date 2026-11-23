//! Teensy 4.1 board support: RTIC app, clocks, displays, and transports.

mod clock;
pub mod diagnostics;
mod display;
mod hardware;
mod midi;
mod rtic_app;
pub mod usb;

pub use clock::SystickClock;
pub use display::run_envelope_display;
pub use hardware::{
    handle_buttons_gpio3_interrupt, handle_buttons_gpio4_interrupt, handle_encoder_interrupt,
    read_sliders,
};
pub use midi::{Bus, UartMidiBus};
pub use usb::UsbMidiBus;

pub use rtic_app::app;
