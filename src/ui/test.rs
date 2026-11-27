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
        mode_count: 3,
    }
}

fn snapshot_with_mode(mode: u8, mode_count: u8) -> crate::core::ConsumerSnapshot {
    let mut snapshot = snapshot([None; 8]);
    snapshot.mode = mode;
    snapshot.mode_count = mode_count;
    snapshot
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

fn controller_with_reader() -> (
    Controller,
    rtic_sync::signal::SignalReader<'static, crate::ui::DisplaySnapshot>,
) {
    let (writer, reader) = Box::leak(Box::new(Signal::new())).split();
    let ui = Controller::new(
        writer,
        crate::core::Selection {
            voice: VoiceId(0),
            lane: 2,
        },
    );
    (ui, reader)
}

#[test]
fn publish_writes_only_when_the_display_changes() {
    let (mut ui, mut reader) = controller_with_reader();
    let target = snapshot([Some(0.5), None, None, None, None, None, None, None]);

    ui.publish(Some(target));
    assert!(reader.try_read().is_some());

    ui.publish(Some(target));
    assert!(reader.try_read().is_none());

    ui.publish(Some(snapshot([Some(0.25), None, None, None, None, None, None, None])));
    assert!(reader.try_read().is_some());
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
        assert_eq!(ui.selected().voice, VoiceId(0));
        let pressed = ui.handle(
            UiEvent::Button { index: 1, pressed: true },
            None,
            Some(2),
            Some(4),
        );
        assert_eq!(ui.selected().voice, VoiceId(0));
        let released = ui.handle(
            UiEvent::Button { index: 1, pressed: false },
            None,
            Some(2),
            Some(4),
        );
        assert_eq!(ui.selected(), crate::core::Selection { voice: VoiceId(1), lane: 2 });
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
    assert_eq!(ui.selected(), crate::core::Selection { voice: VoiceId(0), lane: 3 });
}

#[test]
fn slider_after_voice_selection_targets_the_new_voice() {
    let mut ui = controller();
    let _ = ui.handle(
        UiEvent::EncoderClick { pressed: true },
        None,
        Some(2),
        Some(4),
    );
    let _ = ui.handle(UiEvent::Button { index: 1, pressed: true }, None, Some(2), Some(4));
    let _ = ui.handle(UiEvent::Button { index: 1, pressed: false }, None, Some(2), Some(4));
    assert_eq!(ui.selected().voice, VoiceId(1));

    let updates = ui.handle(
        UiEvent::Slider {
            index: 0,
            value: u16::MAX,
        },
        None,
        Some(2),
        Some(4),
    );
    assert!(matches!(
        updates.first(),
        Some(crate::core::Input::ControlUpdate {
            voice: VoiceId(1),
            lane: 2,
            ..
        })
    ));
}

#[test]
fn encoder_wraps_the_mode_index_in_the_ui() {
    let mut ui = controller();
    // Mode 2 of 3, incrementing wraps to 0.
    let at_last = snapshot_with_mode(2, 3);
    let updates = ui.handle(
        UiEvent::Encoder { increment: true },
        Some(at_last),
        Some(2),
        Some(4),
    );
    assert!(matches!(
        updates.first(),
        Some(crate::core::Input::ModeSet {
            voice: VoiceId(0),
            lane: 2,
            index: 0
        })
    ));

    // Mode 0, decrementing wraps to the last index.
    let at_first = snapshot_with_mode(0, 3);
    let updates = ui.handle(
        UiEvent::Encoder { increment: false },
        Some(at_first),
        Some(2),
        Some(4),
    );
    assert!(matches!(
        updates.first(),
        Some(crate::core::Input::ModeSet { index: 2, .. })
    ));
}

#[test]
fn encoder_does_nothing_for_consumers_without_modes() {
    let mut ui = controller();
    let no_modes = snapshot_with_mode(0, 0);
    let updates = ui.handle(
        UiEvent::Encoder { increment: true },
        Some(no_modes),
        Some(2),
        Some(4),
    );
    assert!(updates.is_empty());
}

#[test]
fn slider_pickup_uses_previous_physical_position() {
    let mut ui = controller();
    let target = snapshot([Some(0.5), None, None, None, None, None, None, None]);

    // Start far from the target so the first move is filtered out.
    let blocked = ui.handle(UiEvent::Slider { index: 0, value: 0 }, Some(target), Some(2), Some(4));
    assert!(blocked.is_empty());

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
        accepted.first(),
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
        decay.first(),
        Some(crate::core::Input::ControlUpdate { control: 5, .. })
    ));

    let attack = ui.handle(UiEvent::Slider { index: 3, value: 0 }, Some(target), Some(2), Some(4));
    assert!(matches!(
        attack.first(),
        Some(crate::core::Input::ControlUpdate { control: 6, .. })
    ));

    let sustain = ui.handle(UiEvent::Slider { index: 1, value: 0 }, Some(target), Some(2), Some(4));
    assert!(matches!(
        sustain.first(),
        Some(crate::core::Input::ControlUpdate { control: 1, .. })
    ));
}

#[test]
fn usb_status_publishes_without_changing_selection() {
    let mut ui = controller();
    let updates = ui.handle(
        UiEvent::UsbConnection { connected: true },
        None,
        Some(2),
        Some(4),
    );
    assert!(updates.is_empty());
    assert_eq!(ui.selected(), crate::core::Selection { voice: VoiceId(0), lane: 2 });
}

#[test]
fn envelope_mode_cycles() {
    assert_eq!(EnvelopeMode::Ad.step(1), EnvelopeMode::Ar);
    assert_eq!(EnvelopeMode::Ar.step(1), EnvelopeMode::Adsr);
    assert_eq!(EnvelopeMode::Adsr.step(1), EnvelopeMode::Ad);
    assert_eq!(EnvelopeMode::Ad.step(-1), EnvelopeMode::Adsr);
}
