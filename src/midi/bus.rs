//! Platform-independent MIDI bus interface.

use enum_dispatch::enum_dispatch;
use midi_msg::MidiMsg;

/// Common interface for MIDI bus implementations (UART, USB, etc.)
#[enum_dispatch]
pub trait MidiBus {
    /// Process incoming MIDI (called from ISR or polling loop)
    fn poll(&mut self);

    /// Send MIDI message to output
    fn send(&mut self, msg: &MidiMsg);
}
