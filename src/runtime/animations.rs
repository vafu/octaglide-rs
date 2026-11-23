use crate::{
    core::{AnimationEngine, Clock, ANIMATORS_PER_VOICE, MAX_VOICES},
    output::{OutputMessage, OutputSender},
};

pub async fn run_animations<C: Clock>(
    engines: [[AnimationEngine; ANIMATORS_PER_VOICE]; MAX_VOICES],
    sender: OutputSender,
) {
    let [first, second, third, fourth] = engines;
    futures::join!(
        run_voice::<C>(first, sender.clone()),
        run_voice::<C>(second, sender.clone()),
        run_voice::<C>(third, sender.clone()),
        run_voice::<C>(fourth, sender),
    );
}

async fn run_voice<C: Clock>(engines: [AnimationEngine; ANIMATORS_PER_VOICE], sender: OutputSender) {
    let [glide, first, second] = engines;
    futures::join!(
        run_engine::<C>(glide, sender.clone()),
        run_engine::<C>(first, sender.clone()),
        run_engine::<C>(second, sender),
    );
}

async fn run_engine<C: Clock>(mut engine: AnimationEngine, mut sender: OutputSender) {
    loop {
        if let Some(messages) = engine.tick::<C>().await {
            for msg in messages {
                sender.send(OutputMessage::Semantic(msg)).await.ok();
            }
        }
    }
}
