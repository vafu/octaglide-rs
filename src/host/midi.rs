//! Host MIDI output bus.

use midi_msg::MidiMsg;

use crate::midi::MidiBus;

/// Logs outgoing MIDI. Incoming notes enter through `AppEvent::Midi`.
pub struct HostMidiBus;

impl HostMidiBus {
    pub fn new() -> Self {
        Self
    }
}

impl MidiBus for HostMidiBus {
    fn poll(&mut self) {}

    fn send(&mut self, msg: &MidiMsg) {
        log::info!("host MIDI out: {:?}", msg);
    }
}
