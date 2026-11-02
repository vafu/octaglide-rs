/// Interpolate an envelope ramp with a consistent visual curve direction.
/// High curve values bend above the linear ramp for rising and falling stages;
/// low values bend below it. Curve 64 is linear. Endpoints remain unchanged.
pub fn envelope_level(progress: f32, curve: u8, from: f32, to: f32) -> f32 {
    let offset = curve as f32 - 64.0;
    let signed_offset = if to < from { offset } else { -offset };
    let exponent = libm::powf(2.0, signed_offset / 32.0);
    let shaped = libm::powf(progress.clamp(0.0, 1.0), exponent);
    from + shaped * (to - from)
}
