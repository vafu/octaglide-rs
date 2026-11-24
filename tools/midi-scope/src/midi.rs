//! MIDI input: attaches to a named port and decodes control changes.

use midi_msg::{ChannelVoiceMsg, MidiMsg};
use midir::{MidiInput, MidiInputConnection};

use crate::trace::Trace;

/// A decoded control change on a specific channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlChange {
    pub channel: u8,
    pub control: u8,
    pub value: u8,
}

/// Decodes one raw MIDI message, returning any control changes it carries.
pub fn decode_control_changes(bytes: &[u8]) -> Vec<ControlChange> {
    let mut context = midi_msg::ReceiverContext::new();
    let mut out = Vec::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        match MidiMsg::from_midi_with_context(rest, &mut context) {
            Ok((msg, len)) => {
                if let MidiMsg::ChannelVoice { channel, msg } = msg
                    && let ChannelVoiceMsg::ControlChange {
                        control: midi_msg::ControlChange::CC { control, value },
                    } = msg
                {
                    out.push(ControlChange {
                        channel: channel as u8,
                        control,
                        value,
                    });
                }
                rest = &rest[len..];
            }
            Err(_) => break,
        }
    }
    out
}

/// Opens a connection to the first input port whose name matches `port`.
///
/// Returns the connection (kept alive by the caller) and the port's name.
pub fn open<F>(port: &str, callback: F) -> Result<(MidiInputConnection<()>, String), String>
where
    F: FnMut(u64, &[u8], &mut ()) + Send + 'static,
{
    let input = MidiInput::new("midi-scope").map_err(|error| error.to_string())?;
    let ports = input.ports();
    let (index, name) = ports
        .iter()
        .enumerate()
        .find_map(|(index, candidate)| {
            let name = input.port_name(candidate).ok()?;
            name.contains(port).then_some((index, name))
        })
        .ok_or_else(|| {
            let available: Vec<String> = ports
                .iter()
                .filter_map(|candidate| input.port_name(candidate).ok())
                .collect();
            format!("no MIDI input port matching '{port}'; available: {available:?}")
        })?;
    let connection = input
        .connect(
            &ports[index],
            "midi-scope-read",
            callback,
            (),
        )
        .map_err(|error| error.to_string())?;
    Ok((connection, name))
}

/// Keeps a trace updated from decoded control changes.
pub struct CcTracker {
    pub trace: Trace,
    pub cc: u8,
    pub channel: u8,
}

impl CcTracker {
    pub fn new(cc: u8, channel: u8, window_secs: f64) -> Self {
        Self {
            trace: Trace::new(window_secs),
            cc,
            channel,
        }
    }

    /// Applies one control change. Returns true when it matched this tracker.
    pub fn apply(&mut self, change: ControlChange, now: f64) -> bool {
        if change.channel != self.channel || change.control != self.cc {
            return false;
        }
        self.trace.push(now, change.value as f32);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_a_control_change() {
        // 0xB0 = control change, channel 1; controller 3; value 42.
        let changes = decode_control_changes(&[0xB0, 0x03, 0x2A]);
        assert_eq!(
            changes,
            vec![ControlChange {
                channel: 0,
                control: 3,
                value: 42
            }]
        );
    }

    #[test]
    fn decodes_control_change_on_a_later_channel() {
        // 0xB3 = channel 4.
        let changes = decode_control_changes(&[0xB3, 0x04, 0x7F]);
        assert_eq!(changes[0].channel, 3);
        assert_eq!(changes[0].control, 4);
        assert_eq!(changes[0].value, 127);
    }

    #[test]
    fn ignores_notes() {
        let changes = decode_control_changes(&[0x90, 0x3C, 0x64]);
        assert!(changes.is_empty());
    }

    #[test]
    fn decodes_multiple_messages_in_one_buffer() {
        let changes = decode_control_changes(&[0xB0, 0x03, 0x01, 0xB0, 0x03, 0x02]);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[1].value, 2);
    }

    #[test]
    fn tracker_ignores_other_controllers() {
        let mut tracker = CcTracker::new(3, 0, 10.0);
        assert!(!tracker.apply(
            ControlChange {
                channel: 0,
                control: 4,
                value: 10
            },
            0.0
        ));
        assert!(tracker.trace.is_empty());
    }

    #[test]
    fn tracker_ignores_other_channels() {
        let mut tracker = CcTracker::new(3, 0, 10.0);
        assert!(!tracker.apply(
            ControlChange {
                channel: 1,
                control: 3,
                value: 10
            },
            0.0
        ));
        assert!(tracker.trace.is_empty());
    }

    #[test]
    fn tracker_records_matching_changes() {
        let mut tracker = CcTracker::new(3, 0, 10.0);
        assert!(tracker.apply(
            ControlChange {
                channel: 0,
                control: 3,
                value: 55
            },
            1.0
        ));
        assert_eq!(tracker.trace.samples(), &[(1.0, 55.0)]);
    }
}
