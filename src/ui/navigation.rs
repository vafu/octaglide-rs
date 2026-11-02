use crate::core::{Consumer, Input, OutputEvents, TrackId};

use super::UiConsumer;

impl UiConsumer {
    pub const fn new() -> Self {
        Self {
            encoder_held: false,
            captured_buttons: 0,
            armed_buttons: 0,
            lane_buttons: 0,
            pending_track: None,
            pending_lane: None,
            sliders: [None; 4],
        }
    }

    /// Take a completed selection gesture for Core to resolve against its tracks.
    pub fn take_track_selection(&mut self) -> Option<TrackId> {
        self.pending_track.take()
    }

    /// Take an unmodified button click selecting a modulation lane.
    pub fn take_lane_selection(&mut self) -> Option<usize> {
        self.pending_lane.take()
    }

    /// Latest physical positions, observed before any per-consumer pickup filter.
    pub(crate) fn slider_context(&self) -> ([Option<u16>; 4], bool) {
        (self.sliders, self.encoder_held)
    }

    /// Apply the current momentary modifier before routing a slider to a voice.
    /// The mapping travels with the edit, so it cannot stick on another page.
    pub fn map_hardware_input(&self, input: &mut Input) {
        if let Input::Slider { alternate, .. } = input {
            *alternate = self.encoder_held;
        }
    }
}

impl Consumer for UiConsumer {
    async fn consume(&mut self, input: &Input, _out: &mut OutputEvents) -> bool {
        match *input {
            Input::Slider { index, value, .. } => {
                if let Some(position) = self.sliders.get_mut(index as usize) {
                    *position = Some(value);
                }
                false
            }
            Input::EncoderClick { pressed: true } => {
                self.lane_buttons = 0;
                self.encoder_held = true;
                true
            }
            Input::EncoderClick { pressed: false } => {
                self.encoder_held = false;
                self.armed_buttons = 0;
                true
            }
            Input::Button { index, pressed } if index < 4 => {
                let mask = 1 << index;
                if pressed {
                    if self.encoder_held {
                        self.captured_buttons |= mask;
                        self.armed_buttons |= mask;
                        true
                    } else {
                        self.lane_buttons |= mask;
                        true
                    }
                } else {
                    let captured = self.captured_buttons & mask != 0;
                    let armed = self.armed_buttons & mask != 0;
                    let lane = self.lane_buttons & mask != 0;
                    self.captured_buttons &= !mask;
                    self.armed_buttons &= !mask;
                    self.lane_buttons &= !mask;
                    if armed && self.encoder_held {
                        self.pending_track = Some(TrackId(index));
                    } else if lane && !captured && !self.encoder_held {
                        self.pending_lane = Some(index as usize);
                    }
                    true
                }
            }
            _ => false,
        }
    }
}
