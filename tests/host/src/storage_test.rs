use crate::runtime::{ANIMATORS_PER_VOICE, MAX_VOICES, RuntimeStorage};

#[test]
fn split_produces_channels_and_engines_for_every_voice() {
    let storage: &'static mut RuntimeStorage = Box::leak(Box::new(RuntimeStorage::new()));
    let handles = storage.split();
    assert_eq!(handles.animators.len(), MAX_VOICES);
    assert_eq!(handles.animators[0].envelopes.len(), 2);
    assert_eq!(handles.engines.len(), MAX_VOICES);
    assert_eq!(handles.engines[0].len(), ANIMATORS_PER_VOICE);
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
        lane: 2,
        usb_connected: false,
        model: None,
    };
    handles.ui_writer.write(snapshot);
    assert_eq!(handles.ui_reader.try_read(), Some(snapshot));
}
