//! Keyboard-to-application event mapping.

use midi_msg::{Channel, ChannelVoiceMsg, MidiMsg};

use crate::{
    app_event::AppEvent,
    ui::{ButtonEdge, UiEvent},
};

/// Logical host controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostKey {
    Button(u8),
    EncoderCw,
    EncoderCcw,
    EncoderClick,
    SliderUp(u8),
    SliderDown(u8),
    Note(u8),
    UsbToggle,
}

/// What a key press should be dispatched to.
#[derive(Clone, Debug, PartialEq)]
pub enum HostEvent {
    App(AppEvent),
    Button(ButtonEdge),
    Encoder(UiEvent),
}

/// Holds per-key state so sliders can accumulate.
pub struct HostInput {
    sliders: [u16; 4],
    velocity: u8,
}

impl HostInput {
    pub fn new() -> Self {
        Self {
            sliders: [u16::MAX / 2; 4],
            velocity: 100,
        }
    }

    pub fn handle(&mut self, key: HostKey, pressed: bool, now_ms: u32) -> Option<HostEvent> {
        match key {
            HostKey::Button(index) => Some(HostEvent::Button(ButtonEdge {
                index,
                pressed,
                at_ms: now_ms,
            })),
            HostKey::EncoderClick => Some(HostEvent::Button(ButtonEdge {
                index: crate::ui::ENCODER_BUTTON_INDEX,
                pressed,
                at_ms: now_ms,
            })),
            HostKey::EncoderCw if pressed => Some(HostEvent::Encoder(UiEvent::Encoder {
                increment: true,
            })),
            HostKey::EncoderCcw if pressed => Some(HostEvent::Encoder(UiEvent::Encoder {
                increment: false,
            })),
            HostKey::SliderUp(index) if pressed => self.slider(index, true),
            HostKey::SliderDown(index) if pressed => self.slider(index, false),
            HostKey::Note(note) => {
                let msg = if pressed {
                    MidiMsg::ChannelVoice {
                        channel: Channel::Ch1,
                        msg: ChannelVoiceMsg::NoteOn {
                            note,
                            velocity: self.velocity,
                        },
                    }
                } else {
                    MidiMsg::ChannelVoice {
                        channel: Channel::Ch1,
                        msg: ChannelVoiceMsg::NoteOff {
                            note,
                            velocity: 0,
                        },
                    }
                };
                Some(HostEvent::App(AppEvent::Midi(msg)))
            }
            HostKey::UsbToggle if pressed => {
                Some(HostEvent::App(AppEvent::Ui(UiEvent::UsbConnection {
                    connected: true,
                })))
            }
            _ => None,
        }
    }

    fn slider(&mut self, index: u8, up: bool) -> Option<HostEvent> {
        let slot = self.sliders.get_mut(index as usize)?;
        *slot = if up {
            slot.saturating_add(4096)
        } else {
            slot.saturating_sub(4096)
        };
        Some(HostEvent::App(AppEvent::Ui(UiEvent::Slider {
            index,
            value: *slot,
        })))
    }
}
