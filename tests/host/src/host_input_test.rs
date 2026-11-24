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

#[test]
fn display_is_split_into_four_slider_strips() {
    use embedded_graphics::geometry::Point;

    // Leftmost strip is Attack (index 3); rightmost is Release (index 0).
    let mut input = HostInput::new();
    for (strip, index) in [3u8, 2, 1, 0].into_iter().enumerate() {
        let x = strip as i32 * 32 + 16;
        match input.pointer_down(Point::new(x, 0)) {
            Some(HostEvent::App(crate::app_event::AppEvent::Ui(
                crate::ui::UiEvent::Slider { index: got, value },
            ))) => {
                assert_eq!(got, index);
                assert_eq!(value, u16::MAX, "top of the strip is full scale");
            }
            other => panic!("unexpected event for strip {index}: {other:?}"),
        }
    }
}

#[test]
fn drag_keeps_the_strip_where_it_started() {
    use embedded_graphics::geometry::Point;

    let mut input = HostInput::new();
    // Leftmost strip is Attack (index 3).
    input.pointer_down(Point::new(10, 32));
    // Move far right; the drag must stay on the strip where it started.
    match input.pointer_move(Point::new(120, 63)) {
        Some(HostEvent::App(crate::app_event::AppEvent::Ui(
            crate::ui::UiEvent::Slider { index, value },
        ))) => {
            assert_eq!(index, 3);
            assert_eq!(value, 0, "bottom of the strip is zero");
        }
        other => panic!("unexpected event: {other:?}"),
    }
    input.pointer_up();
    assert!(input.pointer_move(Point::new(10, 0)).is_none());
}
