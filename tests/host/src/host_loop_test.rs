//! Regression test for the host loop: input must be polled while idle,
//! not only after a display snapshot arrives.

use std::time::Duration;

use crate::{
    app_event::AppEvent,
    runtime::{RuntimeStorage, handle_event},
    core::{Core, Selection, VoiceId},
    midi::MidiController,
    ui::Controller,
};
use futures::executor::block_on;

#[test]
fn midi_note_does_not_publish_a_snapshot() {
    // The host loop must not depend on MIDI producing a snapshot;
    // otherwise it blocks on ui_reader.wait() and never polls SDL input.
    block_on(async {
        let storage: &'static mut RuntimeStorage = Box::leak(Box::new(RuntimeStorage::new()));
        let mut handles = storage.split();
        let mut core = Core::new(handles.output_sender.clone());
        let mut ui = Controller::new(
            handles.ui_writer.clone(),
            Selection { voice: VoiceId(0), slot: 10 },
        );
        ui.publish(core.snapshot(ui.selected().voice, ui.selected().slot));
        assert!(handles.ui_reader.try_read().is_some());

        let note = midi_msg::MidiMsg::ChannelVoice {
            channel: midi_msg::Channel::Ch1,
            msg: midi_msg::ChannelVoiceMsg::NoteOn { note: 60, velocity: 100 },
        };
        handle_event(
            &mut core,
            &mut ui,
            &MidiController::new(),
            &mut handles.output_sender,
            AppEvent::Midi(note),
        )
        .await;

        assert!(handles.ui_reader.try_read().is_none());
    });
}

#[test]
fn polling_times_out_while_no_snapshot_arrives() {
    // Mirrors the host loop's select!: a timer branch must resolve even when
    // no snapshot is ever written.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    runtime.block_on(async {
        let storage: &'static mut RuntimeStorage = Box::leak(Box::new(RuntimeStorage::new()));
        let mut handles = storage.split();
        let mut reader = handles.ui_reader;
        let timed_out = tokio::time::timeout(
            Duration::from_millis(50),
            futures::future::poll_fn(|cx| {
                std::task::Poll::Ready(reader.try_read())
            }),
        )
        .await;
        assert!(timed_out.is_ok(), "poll must not hang without a snapshot");
    });
}
