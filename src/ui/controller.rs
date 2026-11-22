use heapless::Vec;

use crate::{
    core::{ConsumerSnapshot, Input, Selection, VoiceId, MAX_VOICES},
    ui::{DisplaySnapshot, UiWriter},
};

use super::snapshot::model;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Track {
    pub id: u8,
    pub voices: [VoiceId; MAX_VOICES],
    pub count: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiEvent {
    Slider { index: u8, value: u16 },
    Encoder { increment: bool },
    EncoderClick { pressed: bool },
    Button { index: u8, pressed: bool },
    UsbConnection { connected: bool },
}

pub struct UiOutcome {
    pub selection: Option<Selection>,
    pub updates: Vec<Input, MAX_VOICES>,
    pub publish: bool,
}

pub struct Controller {
    writer: UiWriter,
    tracks: [Track; MAX_VOICES],
    selected: Selection,
    encoder_held: bool,
    captured_buttons: u8,
    armed_buttons: u8,
    lane_buttons: u8,
    sliders: [Option<u16>; 4],
    picked_up: [[bool; 2]; 4],
    usb_connected: bool,
}

impl Controller {
    pub fn new(writer: UiWriter, selected: Selection) -> Self {
        let mut tracks = [Track {
            id: 0,
            voices: [VoiceId(0); MAX_VOICES],
            count: 0,
        }; MAX_VOICES];
        for (index, track) in tracks.iter_mut().enumerate() {
            track.id = index as u8;
            track.voices[0] = VoiceId(index as u8);
            track.count = 1;
        }
        Self {
            writer,
            tracks,
            selected,
            encoder_held: false,
            captured_buttons: 0,
            armed_buttons: 0,
            lane_buttons: 0,
            sliders: [None; 4],
            picked_up: [[false; 2]; 4],
            usb_connected: false,
        }
    }

    pub fn reset_pickup(&mut self) {
        self.picked_up = [[false; 2]; 4];
    }

    pub fn publish(&mut self, snapshot: Option<ConsumerSnapshot>) {
        let track = self
            .tracks
            .iter()
            .find(|track| track.voices[..track.count as usize].contains(&self.selected.voice))
            .map_or(0, |track| track.id);
        let display = DisplaySnapshot {
            track,
            voice: self.selected.voice.0,
            lane: self.selected.lane as u8,
            usb_connected: self.usb_connected,
            model: snapshot.as_ref().and_then(model),
        };
        self.writer.write(display);
    }

    pub fn handle(
        &mut self,
        event: UiEvent,
        snapshot: Option<ConsumerSnapshot>,
        first_envelope_lane: Option<usize>,
        lane_count: Option<usize>,
    ) -> UiOutcome {
        let mut outcome = UiOutcome {
            selection: None,
            updates: Vec::new(),
            publish: false,
        };
        match event {
            UiEvent::Slider { index, value } => {
                let Some(slot_index) = (index as usize).checked_sub(0).filter(|i| *i < 4) else {
                    return outcome;
                };
                let alternate = self.encoder_held;
                let Some(slot) = control_slot(slot_index, alternate) else {
                    return outcome;
                };
                let target = snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.controls.get(slot as usize).copied().flatten());
                if !self.accept_slider(slot_index, alternate, value, target) {
                    return outcome;
                }
                outcome
                    .updates
                    .push(Input::ControlUpdate {
                        voice: self.selected.voice,
                        lane: self.selected.lane,
                        control: slot,
                        value: value as f32 / u16::MAX as f32,
                    })
                    .ok();
                outcome.publish = true;
            }
            UiEvent::Encoder { increment } => {
                outcome
                    .updates
                    .push(Input::ModeUpdate {
                        voice: self.selected.voice,
                        lane: self.selected.lane,
                        direction: if increment { 1 } else { -1 },
                    })
                    .ok();
                outcome.publish = true;
            }
            UiEvent::EncoderClick { pressed } => {
                self.encoder_held = pressed;
                if pressed {
                    self.lane_buttons = 0;
                } else {
                    self.armed_buttons = 0;
                }
            }
            UiEvent::Button { index, pressed } if index < 4 => {
                let mask = 1 << index;
                if pressed {
                    if self.encoder_held {
                        self.captured_buttons |= mask;
                        self.armed_buttons |= mask;
                    } else {
                        self.lane_buttons |= mask;
                    }
                } else {
                    let captured = self.captured_buttons & mask != 0;
                    let armed = self.armed_buttons & mask != 0;
                    let lane = self.lane_buttons & mask != 0;
                    self.captured_buttons &= !mask;
                    self.armed_buttons &= !mask;
                    self.lane_buttons &= !mask;
                    if armed && self.encoder_held {
                        if let Some(track) = self.tracks.get(index as usize) {
                            if let Some(&voice) = track.voices[..track.count as usize].first() {
                                if let Some(lane) = first_envelope_lane {
                                    outcome.selection = Some(Selection { voice, lane });
                                }
                            }
                        }
                    } else if lane && !captured && !self.encoder_held {
                        if lane_count.is_some_and(|count| (index as usize) < count) {
                            outcome.selection = Some(Selection {
                                voice: self.selected.voice,
                                lane: index as usize,
                            });
                        }
                    }
                }
            }
            UiEvent::UsbConnection { connected } => {
                self.usb_connected = connected;
                outcome.publish = true;
            }
            _ => {}
        }
        outcome
    }

    fn accept_slider(
        &mut self,
        index: usize,
        alternate: bool,
        value: u16,
        target: Option<f32>,
    ) -> bool {
        let bank = alternate as usize;
        let previous = self.sliders[index].replace(value);
        if (alternate && index != 1) || target.is_none() {
            self.picked_up[index][bank] = true;
            return true;
        }
        // Compare in the consumer's 7-bit domain: the stored setting is a u8,
        // so a u16 round-trip re-arms pickup on a one-count difference.
        let target = libm::roundf(target.unwrap().clamp(0.0, 1.0) * 127.0) as u8;
        let value = libm::roundf(value as f32 * 127.0 / u16::MAX as f32) as u8;
        let previous =
            previous.map(|previous| libm::roundf(previous as f32 * 127.0 / u16::MAX as f32) as u8);
        if self.picked_up[index][bank] && previous.is_some_and(|value| value != target) {
            self.picked_up[index][bank] = false;
        }
        if !self.picked_up[index][bank] {
            let reached = value == target
                || previous.is_some_and(|previous| {
                    (previous.min(value)..=previous.max(value)).contains(&target)
                });
            if !reached {
                return false;
            }
            self.picked_up[index][bank] = true;
        }
        true
    }
}

/// Physical sliders are Release, Sustain, Decay, Attack. The alternate bank
/// reuses Release/Decay/Attack for their curves; Sustain edits Sustain.
fn control_slot(index: usize, alternate: bool) -> Option<u8> {
    if !alternate {
        return Some(index as u8);
    }
    match index {
        0 => Some(4),
        1 => Some(1),
        2 => Some(5),
        3 => Some(6),
        _ => None,
    }
}
