//! The single declaration point for a consumer's controls and modes.
//!
//! One `consumer!` entry generates the control enum, the slot table, the
//! parse/format for each control, the mode helpers, and the `Consumer` impl
//! boilerplate. The consumer still writes `consume_input` (what it does with
//! input) and `after_control_set` (what to do once a setting changed).

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

macro_rules! consumer {
    (
        $consumer:ident {
            settings: $settings:ty,
            wants_notes: $wants_notes:literal,
            $( output_slot: $output_slot:expr, )?
            controls: [
                $( $slot:literal => $control:ident {
                    set: $set:expr,
                    get: $get:expr $(,)?
                } ),* $(,)?
            ]
            $(, modes: {
                count: $mode_count:expr,
                get: $mode_get:expr,
                set: $mode_set:expr $(,)?
            } )? $(,)?
        }
    ) => {
        paste::paste! {
            #[derive(Clone, Copy, Debug, PartialEq, Eq)]
            pub enum [<$consumer Control>] {
                $( $control, )*
            }

            impl $consumer {
                pub(crate) const CONTROL_MAP:
                    [Option<[<$consumer Control>]>; crate::core::CONTROL_SLOTS] =
                    $crate::core::consumers::macros::build_control_map(&[
                        $( ($slot, [<$consumer Control>]::$control) ),*
                    ]);

                pub(crate) fn control_for_slot(slot: u8) -> Option<[<$consumer Control>]> {
                    Self::CONTROL_MAP.get(slot as usize).copied().flatten()
                }

                /// Applies one control's normalized value to the settings.
                fn apply_control(&mut self, control: [<$consumer Control>], value: f32) {
                    let settings: &mut $settings = &mut self.settings;
                    match control {
                        $(
                            [<$consumer Control>]::$control => ($set)(settings, value),
                        )*
                    }
                }

                /// Reads one control's current value as a normalized float.
                fn read_control(&self, control: [<$consumer Control>]) -> f32 {
                    let settings: &$settings = &self.settings;
                    match control {
                        $(
                            [<$consumer Control>]::$control => ($get)(settings),
                        )*
                    }
                }
            }

            impl $crate::core::Consumer for $consumer {
                fn consume(
                    &mut self,
                    input: &$crate::core::Input,
                    out: &mut $crate::core::OutputEvents,
                ) -> $crate::core::Routing {
                    self.consume_input(input, out)
                }

                fn wants_notes(&self) -> bool {
                    $wants_notes
                }

                fn set_control_slot(&mut self, slot: u8, value: f32) {
                    if let Some(control) = Self::control_for_slot(slot) {
                        self.apply_control(control, value);
                        self.after_control_set();
                    }
                }

                fn control_slot_value(&self, slot: u8) -> Option<f32> {
                    Self::control_for_slot(slot).map(|control| self.read_control(control))
                }

                fn output_slot(&self) -> u8 {
                    $( return $output_slot; )?
                    #[allow(unreachable_code)]
                    0
                }

                $(
                    fn mode_count(&self) -> u8 {
                        $mode_count
                    }

                    fn mode_index(&self) -> u8 {
                        ($mode_get)(&self.settings)
                    }

                    fn set_mode(&mut self, index: u8) {
                        if index < $mode_count {
                            ($mode_set)(&mut self.settings, index);
                            self.after_control_set();
                        }
                    }
                )?
            }
        }
    };
}

pub(crate) use consumer;
