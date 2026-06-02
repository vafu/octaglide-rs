mod access;
mod notes;

use heapless::Vec;
use midi_msg::Channel;
use rtic_sync::arbiter::Arbiter;

pub use access::{edit_voice, read_selected_voice, read_voice, route_for_channel, selected_voice};

pub const MAX_VOICES: usize = 4;
pub const MIDI_CHANNELS: usize = 16;
pub const MAX_HELD_NOTES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoiceId(pub u8);

pub struct State {
    pub selected_voice: VoiceId,
    pub routes: [ChannelRoute; MIDI_CHANNELS],
    pub voices: [Voice; MAX_VOICES],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ChannelRoute {
    Off,
    Voice(VoiceId),
    Modifier {
        voice: VoiceId,
        param: ModifierParam,
    },
    External,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ModifierParam {
    Envelope(EnvelopeParam),
    Glide(GlideParam),
    Octave(OctaveParam),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvelopeParam {
    AttackDuration,
    AttackCurve,
    DecayDuration,
    DecayCurve,
    Sustain,
    ReleaseDuration,
    ReleaseCurve,
    Mode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum GlideParam {
    Enabled,
    Duration,
    BendRange,
    NoteOnVelocity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum OctaveParam {
    Shift,
}

#[derive(Debug)]
pub struct Voice {
    pub output_channel: Channel,
    pub envelope: EnvelopeState,
    pub glide: GlideState,
    pub octave: OctaveState,
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
    pub enabled: bool,
    pub duration_ms: u32,
    pub bend_range_semitones: f32,
    pub note_on_velocity: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OctaveState {
    pub shift: i8,
}

#[derive(Debug)]
pub struct HeldNotes {
    pub notes: Vec<u8, MAX_HELD_NOTES>,
    pub bits: [u32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PressResult {
    pub previous_top: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReleaseResult {
    pub was_top: bool,
    pub new_top: Option<u8>,
}

static STATE: Arbiter<State> = Arbiter::new(State {
    selected_voice: VoiceId(0),
    routes: [
        ChannelRoute::Voice(VoiceId(0)),
        ChannelRoute::External,
        ChannelRoute::External,
        ChannelRoute::External,
        ChannelRoute::External,
        ChannelRoute::External,
        ChannelRoute::External,
        ChannelRoute::External,
        ChannelRoute::External,
        ChannelRoute::External,
        ChannelRoute::External,
        ChannelRoute::External,
        ChannelRoute::External,
        ChannelRoute::External,
        ChannelRoute::External,
        ChannelRoute::External,
    ],
    voices: [
        Voice {
            output_channel: Channel::Ch1,
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
                enabled: true,
                duration_ms: 200,
                bend_range_semitones: 2.0,
                note_on_velocity: 100,
            },
            octave: OctaveState { shift: 0 },
            held_notes: HeldNotes {
                notes: Vec::new(),
                bits: [0; 4],
            },
        },
        Voice {
            output_channel: Channel::Ch2,
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
                enabled: true,
                duration_ms: 200,
                bend_range_semitones: 2.0,
                note_on_velocity: 100,
            },
            octave: OctaveState { shift: 0 },
            held_notes: HeldNotes {
                notes: Vec::new(),
                bits: [0; 4],
            },
        },
        Voice {
            output_channel: Channel::Ch3,
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
                enabled: true,
                duration_ms: 200,
                bend_range_semitones: 2.0,
                note_on_velocity: 100,
            },
            octave: OctaveState { shift: 0 },
            held_notes: HeldNotes {
                notes: Vec::new(),
                bits: [0; 4],
            },
        },
        Voice {
            output_channel: Channel::Ch4,
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
                enabled: true,
                duration_ms: 200,
                bend_range_semitones: 2.0,
                note_on_velocity: 100,
            },
            octave: OctaveState { shift: 0 },
            held_notes: HeldNotes {
                notes: Vec::new(),
                bits: [0; 4],
            },
        },
    ],
});

fn channel_index(channel: Channel) -> usize {
    channel as u8 as usize
}

fn voice_index(voice: VoiceId) -> Option<usize> {
    let index = voice.0 as usize;
    (index < MAX_VOICES).then_some(index)
}
