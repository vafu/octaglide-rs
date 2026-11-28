use crate::core::consumers::envelope::EnvelopeMode;
use crate::core::VoiceId;
use crate::ui::Section;
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
            slot: Section::Modulation.slot(2),
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
            slot: Section::Modulation.slot(2),
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
        let _first = ui.handle(
            UiEvent::EncoderClick { pressed: true },
            None,
        );
        assert_eq!(ui.selected().voice, VoiceId(0));
        let pressed = ui.handle(
            UiEvent::Button { index: 1, pressed: true },
            None,
        );
        assert_eq!(ui.selected().voice, VoiceId(0));
        let released = ui.handle(
            UiEvent::Button { index: 1, pressed: false },
            None,
        );
        assert_eq!(
            ui.selected(),
            crate::core::Selection {
                voice: VoiceId(1),
                slot: Section::Modulation.slot(0),
            }
        );
    });
}

#[test]
fn lane_button_without_modifier_selects_lane() {
    let mut ui = controller();
    let _ = ui.handle(
        UiEvent::Button { index: 3, pressed: true },
        None,
    );
    let released = ui.handle(
        UiEvent::Button { index: 3, pressed: false },
        None,
    );
    assert_eq!(ui.selected(), crate::core::Selection { voice: VoiceId(0), slot: Section::Modulation.slot(3) });
}

#[test]
fn slider_after_voice_selection_targets_the_new_voice() {
    let mut ui = controller();
    let _ = ui.handle(
        UiEvent::EncoderClick { pressed: true },
        None,
    );
    let _ = ui.handle(UiEvent::Button { index: 1, pressed: true }, None);
    let _ = ui.handle(UiEvent::Button { index: 1, pressed: false }, None);
    assert_eq!(ui.selected().voice, VoiceId(1));

    let updates = ui.handle(
        UiEvent::Slider {
            index: 0,
            value: u16::MAX,
        },
        None,
    );
    assert!(matches!(
        updates.first(),
        Some(crate::core::Input::ControlUpdate {
            voice: VoiceId(1),
            slot: 8,
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
    );
    assert!(matches!(
        updates.first(),
        Some(crate::core::Input::ModeSet {
            voice: VoiceId(0),
            slot: 10,
            index: 0
        })
    ));

    // Mode 0, decrementing wraps to the last index.
    let at_first = snapshot_with_mode(0, 3);
    let updates = ui.handle(
        UiEvent::Encoder { increment: false },
        Some(at_first),
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
    );
    assert!(updates.is_empty());
}

#[test]
fn slider_pickup_uses_previous_physical_position() {
    let mut ui = controller();
    let target = snapshot([Some(0.5), None, None, None, None, None, None, None]);

    // Start far from the target so the first move is filtered out.
    let blocked = ui.handle(UiEvent::Slider { index: 0, value: 0 }, Some(target));
    assert!(blocked.is_empty());

    // Cross the target between samples; the edit should be accepted.
    let accepted = ui.handle(
        UiEvent::Slider {
            index: 0,
            value: u16::MAX,
        },
        Some(target),
    );
    assert!(matches!(
        accepted.first(),
        Some(crate::core::Input::ControlUpdate {
            voice: VoiceId(0),
            slot: 10,
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
    );
    let target = snapshot([None, None, None, None, None, Some(0.5), Some(0.5), None]);

    let decay = ui.handle(UiEvent::Slider { index: 2, value: 0 }, Some(target));
    assert!(matches!(
        decay.first(),
        Some(crate::core::Input::ControlUpdate { control: 5, .. })
    ));

    let attack = ui.handle(UiEvent::Slider { index: 3, value: 0 }, Some(target));
    assert!(matches!(
        attack.first(),
        Some(crate::core::Input::ControlUpdate { control: 6, .. })
    ));

    let sustain = ui.handle(UiEvent::Slider { index: 1, value: 0 }, Some(target));
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
    );
    assert!(updates.is_empty());
    assert_eq!(ui.selected(), crate::core::Selection { voice: VoiceId(0), slot: Section::Modulation.slot(2) });
}

#[test]
fn envelope_mode_indices_round_trip() {
    for mode in [EnvelopeMode::Ad, EnvelopeMode::Ar, EnvelopeMode::Adsr] {
        assert_eq!(EnvelopeMode::from_index(mode.index()), mode);
    }
}

#[test]
fn idle_encoder_click_cycles_sections() {
    let mut ui = controller();
    assert_eq!(Section::of(ui.selected().slot), Section::Modulation);
    ui.handle(UiEvent::EncoderClick { pressed: true }, None);
    ui.handle(UiEvent::EncoderClick { pressed: false }, None);
    assert_eq!(Section::of(ui.selected().slot), Section::Reserved);
    ui.handle(UiEvent::EncoderClick { pressed: true }, None);
    ui.handle(UiEvent::EncoderClick { pressed: false }, None);
    assert_eq!(Section::of(ui.selected().slot), Section::Sequencing);
}

#[test]
fn chorded_encoder_click_does_not_cycle_sections() {
    let mut ui = controller();
    let before = Section::of(ui.selected().slot);
    ui.handle(UiEvent::EncoderClick { pressed: true }, None);
    ui.handle(UiEvent::Button { index: 0, pressed: true }, None);
    ui.handle(UiEvent::Button { index: 0, pressed: false }, None);
    ui.handle(UiEvent::EncoderClick { pressed: false }, None);
    assert_eq!(Section::of(ui.selected().slot), before, "a chord must not cycle");
}

#[test]
fn slot_button_selects_within_current_section() {
    let mut ui = controller();
    ui.handle(UiEvent::EncoderClick { pressed: true }, None);
    ui.handle(UiEvent::EncoderClick { pressed: false }, None);
    let section = Section::of(ui.selected().slot);
    ui.handle(UiEvent::Button { index: 2, pressed: true }, None);
    ui.handle(UiEvent::Button { index: 2, pressed: false }, None);
    assert_eq!(ui.selected().slot, section.slot(2));
}
