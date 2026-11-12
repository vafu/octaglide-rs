use heapless::Vec;
use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{
    core::{Input, VoiceId},
    output::{OutputMessage, OutputSender},
};

const ROUTES: [(Channel, VoiceId); 8] = [
    (Channel::Ch1, VoiceId(0)),
    (Channel::Ch2, VoiceId(1)),
    (Channel::Ch3, VoiceId(2)),
    (Channel::Ch4, VoiceId(3)),
    (Channel::Ch5, VoiceId(0)),
    (Channel::Ch6, VoiceId(1)),
    (Channel::Ch7, VoiceId(2)),
    (Channel::Ch8, VoiceId(3)),
];

const OUTPUT_CHANNELS: [Channel; 4] = [Channel::Ch1, Channel::Ch2, Channel::Ch3, Channel::Ch4];
const OUTPUT_CC: [[u8; 2]; 4] = [[2, 3], [2, 3], [2, 3], [2, 3]];

#[derive(Debug)]
pub struct MidiController;

impl MidiController {
    pub fn new() -> Self {
        Self
    }

    pub fn translate(&self, msg: &MidiMsg) -> Option<Vec<Input, 4>> {
        let (channel, voice_msg) = match msg {
            MidiMsg::ChannelVoice { channel, msg }
            | MidiMsg::RunningChannelVoice { channel, msg } => (*channel, msg),
            _ => return None,
        };
        let voices = ROUTES
            .iter()
            .filter(|(candidate, _)| *candidate == channel)
            .map(|(_, voice)| *voice);
        let mut out = Vec::new();
        for voice in voices {
            let input = match voice_msg {
                ChannelVoiceMsg::NoteOn { note, velocity } => {
                    if *velocity == 0 {
                        Input::NoteOff { voice, note: *note, velocity: 0 }
                    } else {
                        Input::NoteOn { voice, note: *note, velocity: *velocity }
                    }
                }
                ChannelVoiceMsg::NoteOff { note, velocity } => Input::NoteOff {
                    voice,
                    note: *note,
                    velocity: *velocity,
                },
                _ => return None,
            };
            out.push(input).ok();
        }
        Some(out)
    }

    pub fn is_passthrough(&self, msg: &MidiMsg) -> bool {
        !matches!(msg, MidiMsg::ChannelVoice { .. } | MidiMsg::RunningChannelVoice { .. })
            || self.translate(msg).is_none()
    }

    pub async fn send_raw(&self, msg: MidiMsg, sender: &mut OutputSender) {
        sender.send(OutputMessage::Raw(msg)).await.ok();
    }

    pub fn encode(&self, output: crate::core::VoiceOutput) -> Option<MidiMsg> {
        let channel = |voice: VoiceId| OUTPUT_CHANNELS.get(voice.0 as usize).copied();
        match output {
            crate::core::VoiceOutput::NoteOn { voice, note, velocity } => {
                Some(MidiMsg::ChannelVoice {
                    channel: channel(voice)?,
                    msg: ChannelVoiceMsg::NoteOn { note, velocity },
                })
            }
            crate::core::VoiceOutput::NoteOff { voice, note, velocity } => {
                Some(MidiMsg::ChannelVoice {
                    channel: channel(voice)?,
                    msg: ChannelVoiceMsg::NoteOff { note, velocity },
                })
            }
            crate::core::VoiceOutput::PitchBend { voice, amount } => {
                let bend = (8192.0 + amount.clamp(-1.0, 1.0) * 8191.0)
                    .clamp(0.0, 16383.0) as u16;
                Some(MidiMsg::ChannelVoice {
                    channel: channel(voice)?,
                    msg: ChannelVoiceMsg::PitchBend { bend },
                })
            }
            crate::core::VoiceOutput::Control { voice, slot, value } => {
                let control = *OUTPUT_CC.get(voice.0 as usize)?.get(slot as usize)?;
                Some(MidiMsg::ChannelVoice {
                    channel: channel(voice)?,
                    msg: ChannelVoiceMsg::ControlChange {
                        control: ControlChange::CC {
                            control,
                            value: libm::roundf(value.clamp(0.0, 1.0) * 127.0) as u8,
                        },
                    },
                })
            }
        }
    }
}
