use crate::runtime::{RuntimeStorage};

#[test]
fn split_produces_every_channel_endpoint() {
    let storage: &'static mut RuntimeStorage = Box::leak(Box::new(RuntimeStorage::new()));
    let handles = storage.split();
    assert!(!handles.output_sender.is_closed());
    assert!(!handles.app_sender.is_closed());
}

#[test]
fn cloned_senders_reach_the_same_receiver() {
    let storage: &'static mut RuntimeStorage = Box::leak(Box::new(RuntimeStorage::new()));
    let mut handles = storage.split();
    let mut extra = handles.app_sender.clone();
    extra
        .try_send(crate::app_event::AppEvent::Ui(crate::ui::UiEvent::UsbConnection {
            connected: true,
        }))
        .unwrap();
    assert!(matches!(
        handles.app_receiver.try_recv(),
        Ok(crate::app_event::AppEvent::Ui(crate::ui::UiEvent::UsbConnection {
            connected: true
        }))
    ));
}

#[test]
fn signal_round_trips_a_snapshot() {
    let storage: &'static mut RuntimeStorage = Box::leak(Box::new(RuntimeStorage::new()));
    let mut handles = storage.split();
    let snapshot = crate::ui::DisplaySnapshot {
        track: 2,
        voice: 0,
        section: crate::ui::Section::Modulation,
        lane: 2,
        usb_connected: false,
        model: None,
    };
    handles.ui_writer.write(snapshot);
    assert_eq!(handles.ui_reader.try_read(), Some(snapshot));
}
