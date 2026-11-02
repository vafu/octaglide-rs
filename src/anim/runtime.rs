use super::{ANIMATORS_PER_VOICE, animator::AnimationEngine};
use crate::{
    app::MidiSender,
    core::{MAX_VOICES, MidiOut},
};

/// Run every voice concurrently without allocating or sharing playback state.
pub async fn run_animations(
    engines: [[AnimationEngine; ANIMATORS_PER_VOICE]; MAX_VOICES],
    sender: MidiSender,
) {
    let [first, second, third, fourth] = engines;
    futures::join!(
        run_voice(first, sender.clone()),
        run_voice(second, sender.clone()),
        run_voice(third, sender.clone()),
        run_voice(fourth, sender),
    );
}

async fn run_voice(engines: [AnimationEngine; ANIMATORS_PER_VOICE], sender: MidiSender) {
    let [glide, first, second] = engines;
    futures::join!(
        run_engine(glide, sender.clone(), "glide"),
        run_engine(first, sender.clone(), "env1"),
        run_engine(second, sender, "env2"),
    );
}

async fn run_engine(mut engine: AnimationEngine, mut sender: MidiSender, tag: &'static str) {
    loop {
        if let Some(messages) = engine.tick().await {
            for msg in messages {
                sender.send(MidiOut { msg, tag }).await.ok();
            }
        }
    }
}
