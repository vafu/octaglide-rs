//! Sole drainer of the semantic output queue.

use crate::{
    midi::{MidiBus, MidiController, MidiOutputCache},
    output::{OutputMessage, OutputReceiver},
};

/// Sends every queued message to the platform MIDI bus.
pub async fn run_midi_dispatch<B: MidiBus>(
    bus: &mut B,
    mut receiver: OutputReceiver,
    controller: MidiController,
) -> ! {
    // Dedupe lives here, not in `MidiController`: this task is the only place
    // that sees both what is about to be sent and what the wire already holds.
    let mut cache = MidiOutputCache::new();
    loop {
        let message = receiver.recv().await.unwrap();
        match message {
            OutputMessage::Raw(msg) => {
                cache.record_raw(&msg);
                bus.send(&msg);
            }
            OutputMessage::Semantic(output) => {
                if let Some(msg) = controller.encode(output)
                    && cache.admit_semantic(&msg)
                {
                    bus.send(&msg);
                }
            }
        }
    }
}
