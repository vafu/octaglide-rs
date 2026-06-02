mod channel;
mod notes;

use midi_msg::Channel;
use rtic_sync::arbiter::Arbiter;

pub use channel::{edit_selected_voice, edit_voice, has_voice, read_selected_voice, read_voice};

pub struct State {
    pub selected_channel: Channel,
    pub channels: [ChannelState; 16],
}

#[derive(Debug)]
pub enum ChannelState {
    Off,
    Voice(Voice),
}

#[derive(Debug)]
pub struct Voice {
    pub envelope: EnvelopeState,
    pub glide: GlideState,
    pub held_notes: HeldNotes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnvelopeStageState {
    pub duration: u8,
    pub curve: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnvelopeState {
    pub attack: EnvelopeStageState,
    pub decay: EnvelopeStageState,
    pub release: EnvelopeStageState,
    pub sustain: u8,
    pub mode: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlideState {
    pub duration_ms: u32,
    pub bend_range_semitones: f32,
    pub note_on_velocity: u8,
}

#[derive(Debug)]
pub struct HeldNotes {
    pub words: [u32; 4],
}

static STATE: Arbiter<State> = Arbiter::new(State {
    selected_channel: Channel::Ch1,
    channels: [
        ChannelState::Voice(Voice {
            envelope: EnvelopeState {
                attack: EnvelopeStageState {
                    duration: 20,
                    curve: 64,
                },
                decay: EnvelopeStageState {
                    duration: 20,
                    curve: 64,
                },
                release: EnvelopeStageState {
                    duration: 20,
                    curve: 64,
                },
                sustain: 80,
                mode: 0,
            },
            glide: GlideState {
                duration_ms: 200,
                bend_range_semitones: 2.0,
                note_on_velocity: 100,
            },
            held_notes: HeldNotes { words: [0; 4] },
        }),
        ChannelState::Off,
        ChannelState::Off,
        ChannelState::Off,
        ChannelState::Off,
        ChannelState::Off,
        ChannelState::Off,
        ChannelState::Off,
        ChannelState::Off,
        ChannelState::Off,
        ChannelState::Off,
        ChannelState::Off,
        ChannelState::Off,
        ChannelState::Off,
        ChannelState::Off,
        ChannelState::Off,
    ],
});

fn channel_index(channel: Channel) -> usize {
    channel as u8 as usize
}
