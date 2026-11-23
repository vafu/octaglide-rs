//! Core event loop and per-event dispatch.

use crate::{
    app_event::{AppEvent, AppReceiver},
    core::{Core, Selection, VoiceId},
    midi::MidiController,
    output::{OutputMessage, OutputSender},
    ui::{Controller, UiWriter},
};

/// Handles one application event against Core and the UI controller.
pub async fn handle_event(
    core: &mut Core,
    ui: &mut Controller,
    midi: &MidiController,
    output: &mut OutputSender,
    event: AppEvent,
) {
    match event {
        AppEvent::Midi(msg) => {
            if let Some(inputs) = midi.translate(&msg) {
                for input in inputs {
                    core.process(input).await;
                }
            } else {
                output.send(OutputMessage::Raw(msg)).await.ok();
            }
        }
        AppEvent::Ui(event) => {
            let selected = ui.selected();
            let snapshot = core.snapshot(selected.voice, selected.lane);
            let first_envelope = core.first_envelope_lane(selected.voice);
            let lane_count = core.lane_count(selected.voice);
            let updates = ui.handle(event, snapshot, first_envelope, lane_count);
            for input in updates {
                core.process(input).await;
            }
            ui.publish(core.snapshot(ui.selected().voice, ui.selected().lane));
        }
    }
}

/// Owns Core and the UI selection for the lifetime of the runtime.
pub async fn run_core(
    core: &mut Core,
    mut receiver: AppReceiver,
    mut output: OutputSender,
    ui_writer: UiWriter,
) -> ! {
    let mut ui = Controller::new(
        ui_writer,
        Selection {
            voice: VoiceId(0),
            lane: 2,
        },
    );
    ui.publish(core.snapshot(ui.selected().voice, ui.selected().lane));
    let midi = MidiController::new();
    loop {
        let Ok(event) = receiver.recv().await else {
            continue;
        };
        handle_event(core, &mut ui, &midi, &mut output, event).await;
    }
}
