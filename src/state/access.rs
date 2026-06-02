use midi_msg::Channel;

use super::{ChannelRoute, STATE, Voice, VoiceId, channel_index, voice_index};

pub async fn route_for_channel(channel: Channel) -> ChannelRoute {
    let state = STATE.access().await;
    state.routes[channel_index(channel)]
}

pub async fn selected_voice() -> VoiceId {
    let state = STATE.access().await;
    state.selected_voice
}

pub async fn read_voice<R>(voice: VoiceId, f: impl FnOnce(&Voice) -> R) -> Option<R> {
    let state = STATE.access().await;
    let index = voice_index(voice)?;
    Some(f(&state.voices[index]))
}

pub async fn edit_voice<R>(voice: VoiceId, f: impl FnOnce(&mut Voice) -> R) -> Option<R> {
    let mut state = STATE.access().await;
    let index = voice_index(voice)?;
    Some(f(&mut state.voices[index]))
}

pub async fn read_selected_voice<R>(f: impl FnOnce(&Voice) -> R) -> Option<R> {
    let state = STATE.access().await;
    let index = voice_index(state.selected_voice)?;
    Some(f(&state.voices[index]))
}
