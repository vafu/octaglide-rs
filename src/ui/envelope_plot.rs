use crate::core::EnvelopeParams;

/// Sample the preview on a fixed horizontal time scale, independent of settings.
/// Zero-duration stages are instantaneous; the line stays at zero after completion.
/// Positive stages shorter than one pixel get one pixel so brief attacks remain visible.
/// Hold modes use a fixed illustrative plateau; actual sustain follows note release.
pub(crate) fn envelope_plot_level(envelope: &EnvelopeParams, x: u32) -> f32 {
    crate::core::consumers::envelope::calc::preview_level(envelope, x)
}
