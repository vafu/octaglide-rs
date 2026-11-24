//! Keyboard-to-application event mapping.

use embedded_graphics::geometry::Point;
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
    usb_connected: bool,
    dragging: Option<u8>,
    held_buttons: u8,
    held_notes: [u64; 2],
}

/// Display width split into one vertical strip per physical slider.
const SLIDER_ZONE_WIDTH: i32 = 32;

impl HostInput {
    pub fn new() -> Self {
        Self {
            sliders: [u16::MAX / 2; 4],
            velocity: 100,
            usb_connected: false,
            dragging: None,
            held_buttons: 0,
            held_notes: [0; 2],
        }
    }

    pub fn handle(&mut self, key: HostKey, pressed: bool, now_ms: u32) -> Option<HostEvent> {
        match key {
            HostKey::Button(index) => {
                let mask = 1u8.checked_shl(index as u32).filter(|_| index < 8)?;
                if pressed {
                    if self.held_buttons & mask != 0 {
                        return None;
                    }
                    self.held_buttons |= mask;
                } else {
                    if self.held_buttons & mask == 0 {
                        return None;
                    }
                    self.held_buttons &= !mask;
                }
                Some(HostEvent::Button(ButtonEdge {
                    index,
                    pressed,
                    at_ms: now_ms,
                }))
            }
            HostKey::EncoderClick => {
                let mask = 1u8 << crate::ui::ENCODER_BUTTON_INDEX;
                if pressed {
                    if self.held_buttons & mask != 0 {
                        return None;
                    }
                    self.held_buttons |= mask;
                } else {
                    if self.held_buttons & mask == 0 {
                        return None;
                    }
                    self.held_buttons &= !mask;
                }
                Some(HostEvent::Button(ButtonEdge {
                    index: crate::ui::ENCODER_BUTTON_INDEX,
                    pressed,
                    at_ms: now_ms,
                }))
            }
            HostKey::EncoderCw if pressed => Some(HostEvent::Encoder(UiEvent::Encoder {
                increment: true,
            })),
            HostKey::EncoderCcw if pressed => Some(HostEvent::Encoder(UiEvent::Encoder {
                increment: false,
            })),
            HostKey::SliderUp(index) if pressed => self.slider(index, true),
            HostKey::SliderDown(index) if pressed => self.slider(index, false),
            HostKey::Note(note) => {
                let (word, bit) = note_bit(note)?;
                let mask = 1u64 << bit;
                if pressed {
                    if self.held_notes[word] & mask != 0 {
                        return None;
                    }
                    self.held_notes[word] |= mask;
                } else {
                    if self.held_notes[word] & mask == 0 {
                        return None;
                    }
                    self.held_notes[word] &= !mask;
                }
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
                self.usb_connected = !self.usb_connected;
                Some(HostEvent::App(AppEvent::Ui(UiEvent::UsbConnection {
                    connected: self.usb_connected,
                })))
            }
            _ => None,
        }
    }

    /// Starts a drag in the strip under `point`, like grabbing a physical fader.
    pub fn pointer_down(&mut self, point: Point) -> Option<HostEvent> {
        let index = slider_index(point);
        self.dragging = Some(index);
        self.set_slider(index, slider_value(point.y))
    }

    /// Continues the active drag. Keeps the strip chosen on button-down.
    pub fn pointer_move(&mut self, point: Point) -> Option<HostEvent> {
        let index = self.dragging?;
        self.set_slider(index, slider_value(point.y))
    }

    /// Ends the drag.
    pub fn pointer_up(&mut self) {
        self.dragging = None;
    }

    fn set_slider(&mut self, index: u8, value: u16) -> Option<HostEvent> {
        let slot = self.sliders.get_mut(index as usize)?;
        *slot = value;
        Some(HostEvent::App(AppEvent::Ui(UiEvent::Slider { index, value })))
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

/// Maps a display point to the slider strip containing it.
///
/// The PCB runs Attack, Decay, Sustain, Release left to right (P27..P24),
/// while the firmware indexes Release as 0. So the leftmost strip is the
/// highest index.
fn slider_index(point: Point) -> u8 {
    let strip = (point.x.clamp(0, SLIDER_ZONE_WIDTH * 4 - 1) / SLIDER_ZONE_WIDTH) as u8;
    3 - strip
}

/// Maps vertical position to 0..=u16::MAX, bottom of the panel = 0.
fn slider_value(y: i32) -> u16 {
    let y = y.clamp(0, 63);
    ((63 - y) as u32 * u16::MAX as u32 / 63) as u16
}

/// Bit position for a MIDI note in the held-note set.
fn note_bit(note: u8) -> Option<(usize, u32)> {
    if note > 127 {
        return None;
    }
    Some(((note / 64) as usize, (note % 64) as u32))
}
