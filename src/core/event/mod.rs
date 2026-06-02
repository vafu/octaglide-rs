use heapless::Vec;

use crate::state::{ModifierParam, VoiceId};

pub const EVENT_CAPACITY: usize = 8;
pub const CONTROL_MAX: u16 = u16::MAX;
pub const MIDI_CC_MAX: u16 = 127;
pub const ADC_10BIT_MAX: u16 = 1023;

pub type Events = Vec<Event, EVENT_CAPACITY>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub role: EventRole,
    pub payload: EventPayload,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventRole {
    Voice(VoiceId),
    Modifier {
        voice: VoiceId,
        param: ModifierParam,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventPayload {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8, velocity: u8 },
    Control(u16),
    Delta(i16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardwareControl {
    Slider0,
    Slider1,
    Slider2,
    Slider3,
    Encoder,
    EncoderClick,
}

pub fn normalize_midi_cc(value: u8) -> u16 {
    normalize_u16(value as u16, MIDI_CC_MAX)
}

pub fn normalize_adc_10bit(value: u16) -> u16 {
    normalize_u16(value, ADC_10BIT_MAX)
}

pub fn normalize_u16(value: u16, max: u16) -> u16 {
    if max == 0 {
        return 0;
    }

    ((value.min(max) as u32 * CONTROL_MAX as u32) / max as u32) as u16
}

pub fn control_to_u8(value: u16, min: u8, max: u8) -> u8 {
    let span = max.saturating_sub(min) as u32;
    min.saturating_add(((value as u32 * span) / CONTROL_MAX as u32) as u8)
}

pub fn control_to_u32(value: u16, min: u32, max: u32) -> u32 {
    let span = max.saturating_sub(min);
    min.saturating_add((value as u32 * span) / CONTROL_MAX as u32)
}

pub fn control_to_i8(value: u16, min: i8, max: i8) -> i8 {
    let span = max.saturating_sub(min) as i32;
    min.saturating_add(((value as i32 * span) / CONTROL_MAX as i32) as i8)
}
