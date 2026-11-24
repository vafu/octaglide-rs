//! MIDI input: attaches to a named port and decodes control changes.

use midi_msg::{ChannelVoiceMsg, MidiMsg};
use midir::{MidiInput, MidiInputConnection};


/// A decoded control change on a specific channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlChange {
    pub channel: u8,
    pub control: u8,
    pub value: u8,
}

/// Decodes one raw MIDI message, returning any control changes it carries.
pub fn decode_control_changes(bytes: &[u8]) -> Vec<ControlChange> {
    decode_messages(bytes)
        .into_iter()
        .filter_map(|msg| match msg {
            MidiMsg::ChannelVoice { channel, msg } => match msg {
                ChannelVoiceMsg::ControlChange {
                    control: midi_msg::ControlChange::CC { control, value },
                } => Some(ControlChange {
                    channel: channel as u8,
                    control,
                    value,
                }),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// Decodes every complete MIDI message in `bytes`, dropping unparseable tails.
pub fn decode_messages(bytes: &[u8]) -> Vec<MidiMsg> {
    let mut context = midi_msg::ReceiverContext::new();
    let mut out = Vec::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        match MidiMsg::from_midi_with_context(rest, &mut context) {
            Ok((msg, len)) => {
                out.push(msg);
                rest = &rest[len..];
            }
            Err(_) => break,
        }
    }
    out
}

/// One terminal line per message, e.g. `ch 1 CC 3 = 42`.
pub fn describe(msg: &MidiMsg) -> String {
    match msg {
        MidiMsg::ChannelVoice { channel, msg } => {
            let channel = *channel as u8 + 1;
            match msg {
                ChannelVoiceMsg::NoteOn { note, velocity } => {
                    format!("ch {channel} note on  {note} vel {velocity}")
                }
                ChannelVoiceMsg::NoteOff { note, velocity } => {
                    format!("ch {channel} note off {note} vel {velocity}")
                }
                ChannelVoiceMsg::ControlChange {
                    control: midi_msg::ControlChange::CC { control, value },
                } => format!("ch {channel} CC {control} = {value}"),
                other => format!("ch {channel} {other:?}"),
            }
        }
        other => format!("{other:?}"),
    }
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

/// Filters control changes and forwards matches into ring A.
///
/// Runs on the MIDI callback thread and never blocks or allocates.
pub struct CcFilter {
    pub cc: u8,
    pub channel: u8,
}

impl CcFilter {
    pub fn new(cc: u8, channel: u8) -> Self {
        Self { cc, channel }
    }

    /// Returns the value when this change matches the filter.
    pub fn matches(&self, change: &ControlChange) -> Option<f32> {
        (change.channel == self.channel && change.control == self.cc)
            .then_some(change.value as f32)
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
    fn describes_control_change_for_the_terminal() {
        let messages = decode_messages(&[0xB0, 0x03, 0x2A]);
        assert_eq!(describe(&messages[0]), "ch 1 CC 3 = 42");
    }

    #[test]
    fn describes_notes_for_the_terminal() {
        let messages = decode_messages(&[0x91, 0x3C, 0x64]);
        assert_eq!(describe(&messages[0]), "ch 2 note on  60 vel 100");
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
    fn filter_ignores_other_controllers() {
        let filter = CcFilter::new(3, 0);
        assert_eq!(
            filter.matches(&ControlChange {
                channel: 0,
                control: 4,
                value: 10
            }),
            None
        );
    }

    #[test]
    fn filter_ignores_other_channels() {
        let filter = CcFilter::new(3, 0);
        assert_eq!(
            filter.matches(&ControlChange {
                channel: 1,
                control: 3,
                value: 10
            }),
            None
        );
    }

    #[test]
    fn filter_passes_matching_changes() {
        let filter = CcFilter::new(3, 0);
        assert_eq!(
            filter.matches(&ControlChange {
                channel: 0,
                control: 3,
                value: 55
            }),
            Some(55.0)
        );
    }
}
