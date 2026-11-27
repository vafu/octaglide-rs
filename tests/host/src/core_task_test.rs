use futures::executor::block_on;
use midi_msg::{Channel, ChannelVoiceMsg, MidiMsg};

use crate::{
    app_event::AppEvent,
    core::{Core, Selection, VoiceId},
    midi::MidiController,
    runtime::{RuntimeStorage, handle_event},
    ui::Controller,
};

#[test]
fn midi_note_reaches_core_output() {
    block_on(async {
        let storage: &'static mut RuntimeStorage = Box::leak(Box::new(RuntimeStorage::new()));
        let mut handles = storage.split();
        let mut core = Core::new(handles.output_sender.clone());
        let mut ui = Controller::new(
            handles.ui_writer.clone(),
            Selection {
                voice: VoiceId(0),
                slot: crate::core::Slot::new(crate::core::Section::Modulation, 2),
            },
        );
        let note = MidiMsg::ChannelVoice {
            channel: Channel::Ch1,
            msg: ChannelVoiceMsg::NoteOn {
                note: 60,
                velocity: 100,
            },
        };

        handle_event(
            &mut core,
            &mut ui,
            &MidiController::new(),
            &mut handles.output_sender,
            AppEvent::Midi(note),
        )
        .await;

        assert!(handles.output_receiver.try_recv().is_ok());
    });
}
