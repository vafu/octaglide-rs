//! Core event loop and per-event dispatch.

use futures::future::Either;

use crate::{
    app_event::{AppEvent, AppReceiver},
    core::{Clock, Core, Section, Selection, VoiceId},
    midi::MidiController,
    output::{OutputMessage, OutputSender},
    ui::{Controller, UiWriter},
};

/// Milliseconds between animation ticks.
pub const TICK_MS: u32 = 5;

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
            let snapshot = core.snapshot(selected.voice, selected.slot);
            let updates = ui.handle(event, snapshot);
            for input in updates {
                core.process(input).await;
            }
            ui.publish(core.snapshot(ui.selected().voice, ui.selected().slot));
        }
    }
}

/// Owns Core and the UI selection for the lifetime of the runtime.
pub async fn run_core<C: Clock>(
    core: &mut Core,
    mut receiver: AppReceiver,
    mut output: OutputSender,
    ui_writer: UiWriter,
) -> ! {
    let mut ui = Controller::new(
        ui_writer,
        Selection {
            voice: VoiceId(0),
            slot: crate::core::Slot::new(Section::Modulation, 0),
        },
    );
    ui.publish(core.snapshot(ui.selected().voice, ui.selected().slot));
    let midi = MidiController::new();
    let mut last_tick = C::now_ms();

    loop {
        let receive = receiver.recv();
        let delay = C::delay_ms(TICK_MS);
        futures::pin_mut!(receive, delay);
        match futures::future::select(receive, delay).await {
            Either::Left((event, _)) => {
                if let Ok(event) = event {
                    handle_event(core, &mut ui, &midi, &mut output, event).await;
                }
            }
            Either::Right((_, _)) => {
                let now = C::now_ms();
                let delta = now.wrapping_sub(last_tick);
                last_tick = now;
                core.advance(delta).await;
            }
        }
    }
}
