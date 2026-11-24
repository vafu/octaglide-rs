//! Host MIDI output: a virtual port other software can connect to.

use midi_msg::MidiMsg;
use midir::{MidiOutput, MidiOutputConnection};
#[cfg(unix)]
use midir::os::unix::VirtualOutput;

use crate::midi::MidiBus;

/// Publishes every message on a virtual ALSA sequencer port named `name`.
///
/// Other applications (ShowMIDI, aseqdump, a DAW) can connect to it.
pub struct HostMidiBus {
    connection: Option<MidiOutputConnection>,
    name: String,
}

impl HostMidiBus {
    /// Opens a virtual output port. Falls back to logging if MIDI is unavailable.
    pub fn new(name: &str) -> Self {
        let connection = open_virtual_port(name);
        match &connection {
            Some(_) => log::info!("virtual MIDI port '{name}' is open"),
            None => log::warn!("no virtual MIDI port; falling back to logging only"),
        }
        Self {
            connection,
            name: name.to_owned(),
        }
    }

    pub fn port_name(&self) -> &str {
        &self.name
    }

    pub fn is_open(&self) -> bool {
        self.connection.is_some()
    }
}

impl MidiBus for HostMidiBus {
    fn poll(&mut self) {}

    fn send(&mut self, msg: &MidiMsg) {
        let bytes = msg.to_midi();
        match self.connection.as_mut() {
            Some(connection) => {
                if let Err(error) = connection.send(&bytes) {
                    log::warn!("virtual MIDI send failed: {error}");
                }
            }
            None => log::info!("host MIDI out: {msg:?}"),
        }
    }
}

fn open_virtual_port(name: &str) -> Option<MidiOutputConnection> {
    let output = MidiOutput::new("octaglide").ok()?;
    #[cfg(unix)]
    {
        output.create_virtual(name).ok()
    }
    #[cfg(not(unix))]
    {
        let _ = (output, name);
        None
    }
}
