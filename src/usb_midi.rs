use crate::midi::MidiBus;
use log::info;
use midi_msg::MidiMsg;
use teensy_usbhost as usbhost;

/// USB Host MIDI bus implementation
pub struct UsbMidiBus {}

impl UsbMidiBus {
    pub fn new() -> Self {
        Self {}
    }
}

impl MidiBus for UsbMidiBus {
    fn poll(&mut self) {
        // TODO: Add RX handling when needed (future work)
        panic!("USB Midi Reading is unsupported yet!")
    }

    fn send(&mut self, msg: &MidiMsg) {
        if usbhost::midi_connected() {
            usbhost::send_midi(msg);
        }
    }
}
