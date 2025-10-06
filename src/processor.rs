use midi_msg::{ChannelModeMsg, ChannelVoiceMsg, ControlChange, MidiMsg};

const OCTAVE_OFFSET: i8 = 12;

pub trait MidiProcessor {
    fn process(&mut self, msg: MidiMsg) -> Option<MidiMsg>;
}

pub struct OctaveShifter {
    offset: i8,
}

impl OctaveShifter {
    pub fn new() -> Self {
        OctaveShifter { offset: 0 }
    }
}

impl MidiProcessor for OctaveShifter {
    fn process(&mut self, input_msg: MidiMsg) -> Option<MidiMsg> {
        match input_msg {
            MidiMsg::ChannelVoice { msg, channel }
            | MidiMsg::RunningChannelVoice { msg, channel } => match msg {
                ChannelVoiceMsg::ControlChange { control } => match control {
                    ControlChange::CC { control, value } => {
                        if control == 20 {
                            let octaves = (value as i8 - 64) / 16;
                            self.offset = octaves * OCTAVE_OFFSET;
                            None
                        } else {
                            Some(input_msg)
                        }
                    }
                    _ => None,
                },
                ChannelVoiceMsg::NoteOn { note, velocity }
                | ChannelVoiceMsg::NoteOff { note, velocity } => {
                    let original_note = note as i16;
                    let offset = self.offset as i16;

                    let new_note = (original_note + offset).clamp(0, 127) as u8;

                    let new_voice_msg = if let ChannelVoiceMsg::NoteOn { .. } = msg {
                        ChannelVoiceMsg::NoteOn {
                            note: new_note,
                            velocity,
                        }
                    } else {
                        ChannelVoiceMsg::NoteOff {
                            note: new_note,
                            velocity,
                        }
                    };

                    Some(MidiMsg::ChannelVoice {
                        channel,
                        msg: new_voice_msg,
                    })
                }
                _ => Some(input_msg),
            },
            _ => Some(input_msg),
        }
    }
}
