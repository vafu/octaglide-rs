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
