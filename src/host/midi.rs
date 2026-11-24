//! Host MIDI: a virtual output port other software can monitor, and a virtual
//! input port so external tools (e.g. `aplaymidi`) can drive the simulator.

use midi_msg::MidiMsg;
use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
#[cfg(unix)]
use midir::os::unix::{VirtualInput, VirtualOutput};

use crate::{
    app_event::{AppEvent, AppSender},
    midi::MidiBus,
};

/// Publishes outgoing MIDI and accepts incoming MIDI on virtual ports.
pub struct HostMidiBus {
    output: Option<MidiOutputConnection>,
    input: Option<MidiInputConnection<InputSink>>,
    out_name: String,
    in_name: String,
}

/// State owned by the MIDI input callback thread.
pub struct InputSink {
    sender: AppSender,
    context: midi_msg::ReceiverContext,
}

impl HostMidiBus {
    /// Opens a virtual output port and, when `sender` is given, a virtual input.
    pub fn new(out_name: &str, in_name: &str, sender: AppSender) -> Self {
        let output = open_virtual_output(out_name);
        match &output {
            Some(_) => log::info!("virtual MIDI output '{out_name}' is open"),
            None => log::warn!("no virtual MIDI output; outgoing messages are logged only"),
        }

        let sink = InputSink {
            sender,
            context: midi_msg::ReceiverContext::new(),
        };
        let input = open_virtual_input(in_name, sink);
        match &input {
            Some(_) => log::info!("virtual MIDI input '{in_name}' is open"),
            None => log::warn!("no virtual MIDI input; external playback is unavailable"),
        }

        Self {
            output,
            input,
            out_name: out_name.to_owned(),
            in_name: in_name.to_owned(),
        }
    }

    pub fn output_name(&self) -> &str {
        &self.out_name
    }

    pub fn input_name(&self) -> &str {
        &self.in_name
    }

    pub fn is_output_open(&self) -> bool {
        self.output.is_some()
    }

    pub fn is_input_open(&self) -> bool {
        self.input.is_some()
    }
}

impl MidiBus for HostMidiBus {
    fn poll(&mut self) {}

    fn send(&mut self, msg: &MidiMsg) {
        let bytes = msg.to_midi();
        match self.output.as_mut() {
            Some(connection) => {
                if let Err(error) = connection.send(&bytes) {
                    log::warn!("virtual MIDI send failed: {error}");
                }
            }
            None => log::info!("host MIDI out: {msg:?}"),
        }
    }
}

/// Decodes a buffer of raw MIDI into messages, queuing each for the app.
///
/// The receiver context is kept across calls so running status works.
pub fn dispatch_input(sink: &mut InputSink, bytes: &[u8]) {
    let mut rest = bytes;
    while !rest.is_empty() {
        match MidiMsg::from_midi_with_context(rest, &mut sink.context) {
            Ok((msg, len)) => {
                if sink.sender.try_send(AppEvent::Midi(msg)).is_err() {
                    log::warn!("app queue full; dropped an incoming MIDI message");
                }
                rest = &rest[len..];
            }
            Err(midi_msg::ParseError::UnexpectedEnd) => break,
            Err(error) => {
                log::warn!("incoming MIDI parse error: {error:?}");
                break;
            }
        }
    }
}

fn open_virtual_output(name: &str) -> Option<MidiOutputConnection> {
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

fn open_virtual_input(name: &str, sink: InputSink) -> Option<MidiInputConnection<InputSink>> {
    let input = MidiInput::new("octaglide").ok()?;
    #[cfg(unix)]
    {
        input
            .create_virtual(name, |_timestamp, bytes, sink: &mut InputSink| {
                dispatch_input(sink, bytes)
            }, sink)
            .ok()
    }
    #[cfg(not(unix))]
    {
        let _ = (input, name, sink);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtic_sync::channel::Channel;

    fn sink() -> (&'static mut Channel<AppEvent, 16>, InputSink) {
        let channel: &'static mut Channel<AppEvent, 16> = Box::leak(Box::new(Channel::new()));
        let (sender, _receiver) = channel.split();
        (
            channel,
            InputSink {
                sender,
                context: midi_msg::ReceiverContext::new(),
            },
        )
    }

    #[test]
    fn decodes_a_note_on_from_the_input_port() {
        let (channel, mut sink) = sink();
        dispatch_input(&mut sink, &[0x90, 0x3C, 0x64]);
        assert_eq!(
            channel.ready(),
            1,
            "note-on should be queued for the app"
        );
    }

    #[test]
    fn decodes_running_status_across_buffers() {
        let (channel, mut sink) = sink();
        dispatch_input(&mut sink, &[0x90, 0x3C, 0x64]);
        // Same status byte omitted: running status.
        dispatch_input(&mut sink, &[0x3E, 0x64]);
        assert_eq!(channel.ready(), 2, "running status should decode twice");
    }

    #[test]
    fn ignores_a_truncated_message() {
        let (channel, mut sink) = sink();
        dispatch_input(&mut sink, &[0x90, 0x3C]);
        assert_eq!(channel.ready(), 0, "incomplete messages must not queue");
    }
}
