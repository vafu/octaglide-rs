use crate::host_input::{HostEvent, HostInput, HostKey};
use crate::ui::ButtonEdge;

#[test]
fn note_key_emits_note_on_and_off() {
    let mut input = HostInput::new();
    assert!(matches!(
        input.handle(HostKey::Note(60), true, 0),
        Some(HostEvent::App(_))
    ));
    assert!(matches!(
        input.handle(HostKey::Note(60), false, 1),
        Some(HostEvent::App(_))
    ));
}

#[test]
fn button_key_emits_a_debounced_edge() {
    let mut input = HostInput::new();
    assert!(matches!(
        input.handle(HostKey::Button(0), true, 0),
        Some(HostEvent::Button(ButtonEdge {
            index: 0,
            pressed: true,
            at_ms: 0
        }))
    ));
}

#[test]
fn usb_toggle_flips_both_directions() {
    let mut input = HostInput::new();
    let first = input.handle(HostKey::UsbToggle, true, 0);
    let second = input.handle(HostKey::UsbToggle, true, 1);
    assert!(matches!(
        first,
        Some(HostEvent::App(crate::app_event::AppEvent::Ui(
            crate::ui::UiEvent::UsbConnection { connected: true }
        )))
    ));
    assert!(matches!(
        second,
        Some(HostEvent::App(crate::app_event::AppEvent::Ui(
            crate::ui::UiEvent::UsbConnection { connected: false }
        )))
    ));
}
