use midi_msg::Channel;

use super::{ChannelState, STATE, State, Voice, channel_index};

pub async fn has_voice(channel: Channel) -> bool {
    let state = STATE.access().await;
    voice(&state, channel).is_some()
}

pub async fn read_voice<R>(channel: Channel, f: impl FnOnce(&Voice) -> R) -> Option<R> {
    let state = STATE.access().await;
    voice(&state, channel).map(f)
}

pub async fn edit_voice<R>(channel: Channel, f: impl FnOnce(&mut Voice) -> R) -> Option<R> {
    let mut state = STATE.access().await;
    voice_mut(&mut state, channel).map(f)
}

pub async fn read_selected_voice<R>(f: impl FnOnce(&Voice) -> R) -> Option<R> {
    let state = STATE.access().await;
    voice(&state, state.selected_channel).map(f)
}

pub async fn edit_selected_voice<R>(f: impl FnOnce(&mut Voice) -> R) -> Option<R> {
    let mut state = STATE.access().await;
    let channel = state.selected_channel;
    voice_mut(&mut state, channel).map(f)
}

fn voice(state: &State, channel: Channel) -> Option<&Voice> {
    match &state.channels[channel_index(channel)] {
        ChannelState::Off => None,
        ChannelState::Voice(voice) => Some(voice),
    }
}

fn voice_mut(state: &mut State, channel: Channel) -> Option<&mut Voice> {
    match &mut state.channels[channel_index(channel)] {
        ChannelState::Off => None,
        ChannelState::Voice(voice) => Some(voice),
    }
}
