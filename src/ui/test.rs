use super::ButtonDebouncer;
use super::UiConsumer;
use crate::core::{Consumer, Input, OutputEvents, TrackId};
use futures::executor::block_on;

fn preview_model(mode: u8) -> super::ModTriggerUi {
    let stage = super::EnvelopeStageUi {
        duration: 0,
        curve: 64,
    };
    super::ModTriggerUi {
        name: "Envelope",
        output_cc: 2,
        attack: stage,
        decay: stage,
        sustain: 80,
        release: stage,
        mode,
    }
}

#[test]
fn decay_only_preview_grows_with_absolute_duration_instead_of_filling_the_screen() {
    let mut model = preview_model(0);
    assert!((0..128).all(|x| super::envelope_plot_level(&model, x) == 0.0));
    model.decay.duration = 16;
    let short_end = (0..128)
        .rfind(|&x| super::envelope_plot_level(&model, x) > 0.0)
        .unwrap();
    let short_level = super::envelope_plot_level(&model, 3);
    model.decay.duration = 64;
    let long_end = (0..128)
        .rfind(|&x| super::envelope_plot_level(&model, x) > 0.0)
        .unwrap();
    assert!(long_end > short_end * 3);
    assert!(long_end < 127);
    assert!(super::envelope_plot_level(&model, 3) > short_level);
    assert_eq!(super::envelope_plot_level(&model, long_end + 1), 0.0);
    model.decay.duration = 0;
    model.attack.duration = 1;
    assert!(super::envelope_plot_level(&model, 1) > 0.0);
    assert_eq!(super::envelope_plot_level(&model, 2), 0.0);
}

#[test]
fn preview_attack_uses_the_same_time_scale_across_modes_and_other_stage_values() {
    let mut model = preview_model(0);
    model.attack.duration = 64;
    let level = super::envelope_plot_level(&model, 8);
    for mode in [0, 1, 2] {
        model.mode = mode;
        model.decay.duration = 127;
        model.release.duration = 127;
        assert_eq!(super::envelope_plot_level(&model, 8), level);
    }
    model.mode = 0;
    model.decay.duration = 32;
    let decay_level = super::envelope_plot_level(&model, 25);
    model.attack.duration = 32;
    let shifted_decay_level = super::envelope_plot_level(&model, 15);
    assert!((decay_level - shifted_decay_level).abs() < 0.03);
}

#[test]
fn preview_ignores_unused_stages_and_handles_zero_and_maximum_durations() {
    for mode in [0, 1] {
        let mut model = preview_model(mode);
        model.attack.duration = 32;
        let before: std::vec::Vec<_> = (0..128)
            .map(|x| super::envelope_plot_level(&model, x))
            .collect();
        if mode == 0 {
            model.release.duration = 127;
        } else {
            model.decay.duration = 127;
        }
        for (x, expected) in before.into_iter().enumerate() {
            assert_eq!(super::envelope_plot_level(&model, x as u32), expected);
        }
    }
    for mode in [0, 1, 2] {
        for duration in [0, 127] {
            for curve in [0, 64, 127] {
                let mut model = preview_model(mode);
                let stage = super::EnvelopeStageUi { duration, curve };
                model.attack = stage;
                model.decay = stage;
                model.release = stage;
                for x in 0..128 {
                    let level = super::envelope_plot_level(&model, x);
                    assert!(level.is_finite() && (0.0..=1.0).contains(&level));
                }
                assert_eq!(super::envelope_plot_level(&model, 127), 0.0);
            }
        }
    }
}

#[test]
fn plain_button_clicks_select_lanes_and_cancelled_track_chords_do_not() {
    block_on(async {
        let mut ui = UiConsumer::new();
        let mut out = OutputEvents::new();
        for index in 0..4 {
            assert!(
                ui.consume(
                    &Input::Button {
                        index,
                        pressed: true
                    },
                    &mut out
                )
                .await
            );
            assert_eq!(ui.take_lane_selection(), None);
            assert!(
                ui.consume(
                    &Input::Button {
                        index,
                        pressed: false
                    },
                    &mut out
                )
                .await
            );
            assert_eq!(ui.take_lane_selection(), Some(index as usize));
            assert_eq!(ui.take_track_selection(), None);
            ui.consume(
                &Input::Button {
                    index,
                    pressed: false,
                },
                &mut out,
            )
            .await;
            assert_eq!(ui.take_lane_selection(), None);
        }
        ui.consume(&Input::EncoderClick { pressed: true }, &mut out)
            .await;
        ui.consume(
            &Input::Button {
                index: 3,
                pressed: true,
            },
            &mut out,
        )
        .await;
        ui.consume(&Input::EncoderClick { pressed: false }, &mut out)
            .await;
        ui.consume(
            &Input::Button {
                index: 3,
                pressed: false,
            },
            &mut out,
        )
        .await;
        assert_eq!(ui.take_track_selection(), None);
        assert_eq!(ui.take_lane_selection(), None);
        assert!(out.is_empty());
    });
}

#[test]
fn navigation_only_selects_on_button_release_while_encoder_is_held() {
    block_on(async {
        let mut ui = UiConsumer::new();
        let mut out = OutputEvents::new();
        for index in 0..4 {
            assert!(
                ui.consume(&Input::EncoderClick { pressed: true }, &mut out)
                    .await
            );
            assert!(
                ui.consume(
                    &Input::Button {
                        index,
                        pressed: true
                    },
                    &mut out
                )
                .await
            );
            assert_eq!(ui.take_track_selection(), None);
            assert!(
                ui.consume(
                    &Input::Button {
                        index,
                        pressed: false
                    },
                    &mut out
                )
                .await
            );
            assert_eq!(ui.take_track_selection(), Some(TrackId(index)));
            assert_eq!(ui.take_track_selection(), None);
            assert!(
                ui.consume(&Input::EncoderClick { pressed: false }, &mut out)
                    .await
            );
        }
        assert!(out.is_empty());
    });
}

#[test]
fn navigation_cancels_chords_on_encoder_release_and_requires_a_new_button_press() {
    block_on(async {
        let mut ui = UiConsumer::new();
        let mut out = OutputEvents::new();
        for input in [
            Input::Button {
                index: 2,
                pressed: true,
            }, // Pressed before the modifier.
            Input::EncoderClick { pressed: true },
            Input::Button {
                index: 2,
                pressed: false,
            },
        ] {
            ui.consume(&input, &mut out).await;
            assert_eq!(ui.take_track_selection(), None);
        }
        ui.consume(
            &Input::Button {
                index: 1,
                pressed: true,
            },
            &mut out,
        )
        .await;
        assert!(
            ui.consume(&Input::EncoderClick { pressed: false }, &mut out)
                .await
        );
        // Re-holding the encoder must not re-arm a previously captured button.
        ui.consume(&Input::EncoderClick { pressed: true }, &mut out)
            .await;
        assert!(
            ui.consume(
                &Input::Button {
                    index: 1,
                    pressed: false
                },
                &mut out
            )
            .await
        );
        assert_eq!(ui.take_track_selection(), None);
        ui.consume(
            &Input::Button {
                index: 1,
                pressed: false,
            },
            &mut out,
        )
        .await;
        assert_eq!(ui.take_track_selection(), None);
    });
}

#[test]
fn navigation_passes_unhandled_events_and_consumes_both_encoder_switch_edges() {
    block_on(async {
        let mut ui = UiConsumer::new();
        let mut out = OutputEvents::new();
        for input in [
            Input::Encoder { increment: true },
            Input::Slider {
                index: 3,
                value: 123,
                alternate: false,
            },
            Input::Button {
                index: 255,
                pressed: true,
            },
            Input::UsbConnection { connected: true },
            Input::Midi(midi_msg::MidiMsg::SystemRealTime {
                msg: midi_msg::SystemRealTimeMsg::TimingClock,
            }),
        ] {
            assert!(!ui.consume(&input, &mut out).await);
        }
        assert!(
            ui.consume(&Input::EncoderClick { pressed: true }, &mut out)
                .await
        );
        assert!(
            ui.consume(&Input::EncoderClick { pressed: false }, &mut out)
                .await
        );
        assert!(
            ui.consume(&Input::EncoderClick { pressed: false }, &mut out)
                .await
        );
        assert!(out.is_empty());
        assert_eq!(ui.take_track_selection(), None);
    });
}

#[test]
fn settled_releases_preserve_modifier_order_instead_of_button_index_order() {
    let mut buttons = [const { ButtonDebouncer::new() }; 5];
    assert!(buttons[4].update(0, true));
    assert!(buttons[0].update(1, true));
    buttons[4].update(10, false); // Encoder released first.
    buttons[0].update(12, false);
    assert_eq!(super::buttons::take_next_release(&mut buttons, 39), None);
    assert_eq!(super::buttons::take_next_release(&mut buttons, 45), Some(4));
    assert_eq!(super::buttons::take_next_release(&mut buttons, 45), Some(0));
    assert_eq!(super::buttons::take_next_release(&mut buttons, 45), None);
    // If dispatch is delayed, settle the previous release before a queued press.
    assert!(buttons[0].update(50, true));
    buttons[0].update(60, false);
    assert_eq!(
        super::buttons::take_next_release(&mut buttons, 100),
        Some(0)
    );
    assert!(buttons[0].update(100, true));
}

#[test]
fn button_emits_once_while_held_until_a_stable_release() {
    let mut button = ButtonDebouncer::new();
    assert!(button.update(0, true));
    assert!(!button.update(30, true));
    assert!(!button.update(1000, true));
    assert!(!button.update(1010, false));
    assert!(!button.take_release(1011));
    assert!(!button.update(1015, true)); // Release bounce must not re-arm.
    assert!(!button.update(1020, false));
    assert!(!button.update(1049, true));
    assert!(!button.update(1050, false));
    assert!(!button.take_release(1079));
    assert!(button.take_release(1080));
    assert!(!button.take_release(1080));
    assert!(button.update(1080, true));
    assert!(!button.update(2000, true));
}

#[test]
fn button_release_latches_are_independent_and_wrap_safely() {
    let mut buttons = [const { ButtonDebouncer::new() }; 4];
    for button in &mut buttons {
        assert!(!button.update(u32::MAX - 50, false));
        assert!(button.update(u32::MAX - 40, true));
        assert!(!button.update(u32::MAX - 10, false));
        assert!(!button.update(5, true));
        assert!(!button.update(10, false));
        assert!(button.take_release(40));
        assert!(button.update(40, true));
        assert!(!button.update(1000, true));
    }
}

#[test]
fn button_reports_release_without_another_press_and_ignores_idle_edges() {
    let mut button = ButtonDebouncer::new();
    assert!(!button.update(0, false));
    assert!(!button.release_pending());
    assert!(!button.take_release(100));
    assert!(button.update(101, true));
    assert!(!button.update(u32::MAX - 10, false));
    assert!(!button.take_release(5));
    assert!(button.take_release(20));
    assert!(!button.release_pending());
    assert!(!button.take_release(100));
    assert!(button.update(200, true));
}
