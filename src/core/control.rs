//! Normalize hardware values and scale consumer settings.

const CONTROL_MAX: u16 = u16::MAX;
const ADC_10BIT_MAX: u16 = 1023;

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
