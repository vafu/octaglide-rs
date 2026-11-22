use crate::core::{EnvelopeMode, VoiceId};
use crate::ui::{Controller, UiEvent};
use futures::executor::block_on;
use rtic_sync::signal::Signal;

fn snapshot(controls: [Option<f32>; 8]) -> crate::core::ConsumerSnapshot {
    crate::core::ConsumerSnapshot {
        kind: crate::core::ConsumerKind::ModTrigger,
        output_slot: 0,
        controls,
        mode: 0,
    }
}

fn controller() -> Controller {
    let (writer, _) = Box::leak(Box::new(Signal::new())).split();
    Controller::new(
        writer,
        crate::core::Selection {
            voice: VoiceId(0),
            lane: 2,
        },
    )
}

#[test]
fn encoder_hold_and_button_release_selects_first_envelope() {
    block_on(async {
        let mut ui = controller();
        let first = ui.handle(
            UiEvent::EncoderClick { pressed: true },
            None,
            Some(2),
            Some(4),
        );
        assert!(first.selection.is_none());
        let pressed = ui.handle(
            UiEvent::Button { index: 1, pressed: true },
            None,
            Some(2),
            Some(4),
        );
        assert!(pressed.selection.is_none());
        let released = ui.handle(
            UiEvent::Button { index: 1, pressed: false },
            None,
            Some(2),
            Some(4),
        );
        assert_eq!(released.selection, Some(crate::core::Selection { voice: VoiceId(1), lane: 2 }));
    });
}

#[test]
fn lane_button_without_modifier_selects_lane() {
    let mut ui = controller();
    let _ = ui.handle(
        UiEvent::Button { index: 3, pressed: true },
        None,
        Some(2),
        Some(4),
    );
    let released = ui.handle(
        UiEvent::Button { index: 3, pressed: false },
        None,
        Some(2),
        Some(4),
    );
    assert_eq!(released.selection, Some(crate::core::Selection { voice: VoiceId(0), lane: 3 }));
}

#[test]
fn mode_update_targets_selected_voice_and_lane() {
    let mut ui = controller();
    let outcome = ui.handle(UiEvent::Encoder { increment: true }, None, Some(2), Some(4));
    assert!(matches!(
        outcome.updates.first(),
        Some(crate::core::Input::ModeUpdate {
            voice: VoiceId(0),
            lane: 2,
            direction: 1
        })
    ));
}

#[test]
fn slider_pickup_uses_previous_physical_position() {
    let mut ui = controller();
    let target = snapshot([Some(0.5), None, None, None, None, None, None, None]);

    // Start far from the target so the first move is filtered out.
    let blocked = ui.handle(UiEvent::Slider { index: 0, value: 0 }, Some(target), Some(2), Some(4));
    assert!(blocked.updates.is_empty());

    // Cross the target between samples; the edit should be accepted.
    let accepted = ui.handle(
        UiEvent::Slider {
            index: 0,
            value: u16::MAX,
        },
        Some(target),
        Some(2),
        Some(4),
    );
    assert!(matches!(
        accepted.updates.first(),
        Some(crate::core::Input::ControlUpdate {
            voice: VoiceId(0),
            lane: 2,
            control: 0,
            ..
        })
    ));
}

#[test]
fn alternate_slider_bank_maps_decay_and_attack_curves() {
    let mut ui = controller();
    let _ = ui.handle(
        UiEvent::EncoderClick { pressed: true },
        None,
        Some(2),
        Some(4),
    );
    let target = snapshot([None, None, None, None, None, Some(0.5), Some(0.5), None]);

    let decay = ui.handle(UiEvent::Slider { index: 2, value: 0 }, Some(target), Some(2), Some(4));
    assert!(matches!(
        decay.updates.first(),
        Some(crate::core::Input::ControlUpdate { control: 5, .. })
    ));

    let attack = ui.handle(UiEvent::Slider { index: 3, value: 0 }, Some(target), Some(2), Some(4));
    assert!(matches!(
        attack.updates.first(),
        Some(crate::core::Input::ControlUpdate { control: 6, .. })
    ));

    let sustain = ui.handle(UiEvent::Slider { index: 1, value: 0 }, Some(target), Some(2), Some(4));
    assert!(matches!(
        sustain.updates.first(),
        Some(crate::core::Input::ControlUpdate { control: 1, .. })
    ));
}

#[test]
fn usb_status_publishes_without_changing_selection() {
    let mut ui = controller();
    let outcome = ui.handle(
        UiEvent::UsbConnection { connected: true },
        None,
        Some(2),
        Some(4),
    );
    assert!(outcome.publish);
    assert_eq!(outcome.selection, None);
}

#[test]
fn envelope_mode_cycles() {
    assert_eq!(EnvelopeMode::Ad.step(1), EnvelopeMode::Ar);
    assert_eq!(EnvelopeMode::Ar.step(1), EnvelopeMode::Adsr);
    assert_eq!(EnvelopeMode::Adsr.step(1), EnvelopeMode::Ad);
    assert_eq!(EnvelopeMode::Ad.step(-1), EnvelopeMode::Adsr);
}
