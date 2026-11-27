//! The single declaration point for animator-backed modulations.
//!
//! Each entry derives the consumer's control enum and control table. The
//! registry also records which slots hold timed consumers so Core knows which
//! ones to advance.

/// Places each `(slot, control)` pair into a fixed-size slot table.
///
/// Unlisted slots stay `None`, so callers never spell out the padding.
pub(crate) const fn build_control_map<C: Copy, const N: usize>(
    entries: &[(u8, C)],
) -> [Option<C>; N] {
    let mut map = [None; N];
    let mut index = 0;
    while index < entries.len() {
        let (slot, control) = entries[index];
        assert!((slot as usize) < N);
        map[slot as usize] = Some(control);
        index += 1;
    }
    map
}

macro_rules! modulation {
    (
        $(
            $variant:ident {
                consumer: $consumer:ident,
                controls: [ $( ($control_slot:literal, $control:ident) ),* $(,)? ],
            }
        )*
    ) => {
        $(
            paste::paste! {
                #[derive(Clone, Copy, Debug, PartialEq, Eq)]
                pub enum [<$variant Control>] {
                    $( $control, )*
                }

                impl $consumer {
                    pub(crate) const CONTROL_MAP:
                        [Option<[<$variant Control>]>; crate::core::CONTROL_SLOTS] =
                        $crate::core::consumers::registry::build_control_map(&[
                            $( ($control_slot, [<$variant Control>]::$control) ),*
                        ]);

                    pub(crate) fn control_for_slot(slot: u8) -> Option<[<$variant Control>]> {
                        Self::CONTROL_MAP.get(slot as usize).copied().flatten()
                    }
                }
            }
        )*
    };
}

use super::{Glider, ModTrigger};

// The registry. Adding an animator-backed modulation starts and ends here.
modulation! {
    Glide {
        consumer: Glider,
        controls: [
            (0, Enabled),
            (1, NoteOnVelocity),
            (2, BendRange),
            (3, Duration),
        ],
    }
    Envelope {
        consumer: ModTrigger,
        controls: [
            (0, ReleaseDuration),
            (1, Sustain),
            (2, DecayDuration),
            (3, AttackDuration),
            (4, ReleaseCurve),
            (5, DecayCurve),
            (6, AttackCurve),
            (7, Activation),
        ],
    }
}
