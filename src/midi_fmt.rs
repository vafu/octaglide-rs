use core::fmt;
use midi_msg::{ChannelVoiceMsg, MidiMsg};

/// Wrapper for formatting MIDI messages concisely
pub struct MidiFmt<'a>(pub &'a MidiMsg);

impl<'a> fmt::Display for MidiFmt<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            MidiMsg::ChannelVoice { msg, .. } => match msg {
                ChannelVoiceMsg::NoteOn { note, velocity } => {
                    write!(f, "NoteOn {} vel={}", note, velocity)
                }
                ChannelVoiceMsg::NoteOff { note, velocity } => {
                    write!(f, "NoteOff {} vel={}", note, velocity)
                }
                ChannelVoiceMsg::PitchBend { .. } => write!(f, ""),
                other => write!(f, "{:?}", other),
            },
            other => write!(f, "{:?}", other),
        }
    }
}
