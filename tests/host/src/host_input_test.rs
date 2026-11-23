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
