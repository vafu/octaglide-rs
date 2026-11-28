use crate::core::{ConsumerKind, ConsumerSnapshot};

use crate::core::{EnvelopeMode, EnvelopeStageState, EnvelopeState};
use crate::ui::{GliderUi, ModTriggerUi, OctaveUi, UiModel};

fn u8_value(value: Option<f32>) -> u8 {
    libm::roundf(value.unwrap_or(0.0).clamp(0.0, 1.0) * 127.0) as u8
}

fn stage(duration: Option<f32>, curve: Option<f32>) -> EnvelopeStageState {
    EnvelopeStageState {
        duration: u8_value(duration),
        curve: u8_value(curve),
    }
}

pub fn model(snapshot: &ConsumerSnapshot) -> Option<UiModel> {
    match snapshot.kind {
        ConsumerKind::Noop => None,
        ConsumerKind::Octave => Some(UiModel::Octave(OctaveUi {
            shift: libm::roundf(snapshot.controls[0].unwrap_or(0.5).clamp(0.0, 1.0) * 84.0) as i8
                - 48,
        })),
        ConsumerKind::Glider => Some(UiModel::Glider(GliderUi {
            enabled: snapshot.controls[0].unwrap_or(0.0) >= 0.5,
            duration_ms: libm::roundf(snapshot.controls[3].unwrap_or(0.0) * 4000.0) as u32,
            bend_range_semitones: 1.0 + snapshot.controls[2].unwrap_or(0.0) * 23.0,
            note_on_velocity: libm::roundf(1.0 + snapshot.controls[1].unwrap_or(0.0) * 126.0)
                as u8,
        })),
        ConsumerKind::ModTrigger => Some(UiModel::ModTrigger(ModTriggerUi {
            name: "Envelope",
            output_cc: if snapshot.output_slot == 0 { 2 } else { 3 },
            envelope: EnvelopeState {
                attack: stage(snapshot.controls[3], snapshot.controls[6]),
                decay: stage(snapshot.controls[2], snapshot.controls[5]),
                sustain: u8_value(snapshot.controls[1]),
                release: stage(snapshot.controls[0], snapshot.controls[4]),
                mode: EnvelopeMode::from_index(snapshot.mode),
                // Slot 7 is the trigger control, expressed 0..=1.
                trigger: crate::core::Trigger::from_index(libm::roundf(
                    snapshot.controls[7].unwrap_or(0.0).clamp(0.0, 1.0) * 2.0,
                ) as u8),
            },
        })),
    }
}
