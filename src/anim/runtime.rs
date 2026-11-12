use crate::{
    core::{AnimationEngine, ANIMATORS_PER_VOICE, MAX_VOICES},
    output::{OutputMessage, OutputSender},
};

pub async fn run_animations(
    engines: [[AnimationEngine; ANIMATORS_PER_VOICE]; MAX_VOICES],
    sender: OutputSender,
) {
    let [first, second, third, fourth] = engines;
    futures::join!(
        run_voice(first, sender.clone()),
        run_voice(second, sender.clone()),
        run_voice(third, sender.clone()),
        run_voice(fourth, sender),
    );
}

async fn run_voice(engines: [AnimationEngine; ANIMATORS_PER_VOICE], sender: OutputSender) {
    let [glide, first, second] = engines;
    futures::join!(
        run_engine(glide, sender.clone()),
        run_engine(first, sender.clone()),
        run_engine(second, sender),
    );
}

async fn run_engine(mut engine: AnimationEngine, mut sender: OutputSender) {
    loop {
        if let Some(messages) = engine.tick().await {
            for msg in messages {
                sender.send(OutputMessage::Semantic(msg)).await.ok();
            }
        }
    }
}
