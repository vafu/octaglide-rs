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
pub use midi::UartMidiBus;
pub use usb::UsbMidiBus;

pub use rtic_app::app;
