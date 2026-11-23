//! Sole drainer of the semantic output queue.

use crate::{
    midi::{MidiBus, MidiController},
    output::{OutputMessage, OutputReceiver},
};

/// Sends every queued message to the platform MIDI bus.
pub async fn run_midi_dispatch<B: MidiBus>(
    bus: &mut B,
    mut receiver: OutputReceiver,
    controller: MidiController,
) -> ! {
    loop {
        let message = receiver.recv().await.unwrap();
        match message {
            OutputMessage::Raw(msg) => bus.send(&msg),
            OutputMessage::Semantic(output) => {
                if let Some(msg) = controller.encode(output) {
                    bus.send(&msg);
                }
            }
        }
    }
}
