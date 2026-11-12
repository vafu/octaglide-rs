use core::fmt;
use midi_msg::{ChannelVoiceMsg, MidiMsg};

/// Suppress messages that are too frequent to be useful in normal logs.
pub fn should_log(msg: &MidiMsg) -> bool {
    !matches!(
        msg,
        MidiMsg::ChannelVoice {
            msg: ChannelVoiceMsg::PitchBend { .. },
            ..
        } | MidiMsg::RunningChannelVoice {
            msg: ChannelVoiceMsg::PitchBend { .. },
            ..
        } | MidiMsg::SystemRealTime {
            msg: midi_msg::SystemRealTimeMsg::TimingClock,
        }
    )
}

/// Log MIDI traffic at debug level.
pub fn log_midi(prefix: &str, tag: Option<&str>, msg: &MidiMsg) {
    if !should_log(msg) {
        return;
    }

    match tag {
        Some(tag) => log::debug!("{} [{}] {}", prefix, tag, MidiFmt(msg)),
        None => log::debug!("{} {}", prefix, MidiFmt(msg)),
    }
}

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
                ChannelVoiceMsg::PitchBend { bend } => {
                    write!(f, "PitchBend {}", bend)
                }
                other => write!(f, "{:?}", other),
            },
            other => write!(f, "{:?}", other),
        }
    }
}
