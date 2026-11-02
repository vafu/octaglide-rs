//! Core's public types and track/voice ownership model.

mod consumers;
mod control;
mod core;
mod selection;
mod voice;

use heapless::Vec;
use midi_msg::{Channel, MidiMsg};

use crate::{
    app::MidiSender,
    ui::{UiConsumer, UiWriter},
};

pub(crate) use self::consumers::Consumer;

pub(crate) use crate::state::{EnvelopeStageState, EnvelopeState, GlideState, OctaveState};

use self::consumers::Consumers;

#[cfg(target_os = "none")]
pub(crate) use control::normalize_adc_10bit;
pub(crate) use control::{control_to_i8, control_to_u8, control_to_u32};

/// A MIDI message queued for output, with its logging source.
#[derive(Debug)]
pub struct MidiOut {
    pub msg: MidiMsg,
    pub tag: &'static str,
}

/// Owns track assignments and the voice instances they select by ID.
///
/// Each voice owns its consumer chain and its settings. Construction and
/// processing live in `core.rs`; UI selection and routing are owned here.
pub struct Core {
    ui_consumer: UiConsumer,
    tracks: Vec<Track, MAX_TRACKS>,
    voices: Vec<Voice, MAX_VOICES>,
    midi_sender: MidiSender,
    selected_track: TrackId,
    selected_voice: VoiceId,
    selected_lane: usize,
    ui_writer: UiWriter,
    ui_dirty: bool,
    usb_connected: bool,
}

/// Input to Core: MIDI, hardware actions, or a page selection request.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    Midi(MidiMsg),
    /// Select a modulation lane without transferring ownership to the UI.
    #[allow(dead_code)] // Hardware selection controls will be wired separately.
    Select {
        track: TrackId,
        voice: VoiceId,
        lane: usize,
    },
    /// Slider index 0 through 3 and its normalized absolute value.
    Slider {
        index: u8,
        value: u16,
        /// UI selects the alternate mapping while the encoder switch is held.
        alternate: bool,
    },
    /// One encoder detent: increase when true, decrease when false.
    Encoder {
        increment: bool,
    },
    /// Encoder switch state; UI may consume it as a navigation modifier.
    EncoderClick {
        pressed: bool,
    },
    /// Debounced state of hardware button 0 through 3 (P28 through P31).
    Button {
        index: u8,
        pressed: bool,
    },
    /// Ready MIDI-device status from the USB host port.
    UsbConnection {
        connected: bool,
    },
}

/// Maximum MIDI messages passed between consumers in one processing step.
pub const OUTPUT_CAPACITY: usize = 8;

/// Consumers can generate MIDI output, never hardware or selection actions.
pub type OutputEvents = Vec<MidiMsg, OUTPUT_CAPACITY>;

/// Number of Octatrack-associated tracks in the initial configuration.
pub const MAX_TRACKS: usize = 4;

/// Capacity for the octave/glide processing group before modulation lanes.
pub const MAX_VOICING_CONSUMERS: usize = 2;

/// Every voice always contains exactly four modulation lanes.
pub const MODULATION_LANES: usize = 4;

/// Identifies an input track independently of any destination voice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrackId(pub u8);

/// Input track: channel assignments and references to destination voices.
///
/// Core owns the voice instances; a track selects them by ID so different
/// tracks can share a voice later. Initial assignments are one voice per track.
/// The main and auxiliary channels select this track during MIDI routing.
#[derive(Debug)]
pub struct Track {
    id: TrackId,
    /// Primary input MIDI channel for this track.
    main_channel: Channel,
    /// Optional auxiliary input MIDI channel for this track.
    aux_channel: Option<Channel>,
    voices: Vec<VoiceId, MAX_VOICES>,
}

/// Physical voice: voicing processors followed by exactly four modulation lanes.
///
/// Consumers own their processing state and animation resources. Voice sends
/// any MIDI left after processing on its output channel, including when all
/// modulation lanes are passthrough. Only held notes are shared with animations.
#[derive(Debug)]
pub struct Voice {
    id: VoiceId,
    output_channel: Channel,
    midi_sender: MidiSender,
    voicing: Vec<Consumers, MAX_VOICING_CONSUMERS>,
    modulations: [Consumers; MODULATION_LANES],
}

/// Maximum physical voice count; shared note storage uses the same limit.
pub const MAX_VOICES: usize = 4;

/// Identifies a physical voice independently of its selected input tracks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoiceId(pub u8);

#[cfg(test)]
mod test;
