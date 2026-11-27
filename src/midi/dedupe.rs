//! Suppresses MIDI output that quantises to the value already on the wire.
//!
//! Values leave `Core` as `f32` and only become 7-bit at `MidiController::encode`.
//! Two floats can therefore encode to the same CC byte, which the animator cannot
//! detect. This cache compares what will actually be sent.

use midi_msg::{ChannelVoiceMsg, ControlChange, MidiMsg};

/// Slots for remembering the last CC value per `(channel, control)` pair.
///
/// A raw MIDI stream may touch any controller number, so this is a small
/// fixed-size table rather than an array indexed by controller.
const CC_SLOTS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CcEntry {
    channel: u8,
    control: u8,
    value: u8,
}

/// Remembers the last value sent per control and channel.
#[derive(Debug)]
pub struct MidiOutputCache {
    cc: [Option<CcEntry>; CC_SLOTS],
    bend: [Option<u16>; 16],
}

impl MidiOutputCache {
    pub const fn new() -> Self {
        Self {
            cc: [None; CC_SLOTS],
            bend: [None; 16],
        }
    }

    /// True when a semantic message should be sent.
    ///
    /// Duplicate CC and pitch-bend messages are suppressed and do not update the
    /// cache. Notes and everything else always pass.
    pub fn admit_semantic(&mut self, msg: &MidiMsg) -> bool {
        match control_value(msg) {
            Some((channel, control, value)) => self.admit_cc(channel, control, value),
            None => match bend_value(msg) {
                Some((channel, bend)) => self.admit_bend(channel, bend),
                None => true,
            },
        }
    }

    /// Records a message that bypassed encoding, without suppressing it.
    ///
    /// Raw passthrough must reach the bus even when it repeats, but the cache
    /// still has to learn the new wire value so a later semantic message is
    /// compared against reality.
    pub fn record_raw(&mut self, msg: &MidiMsg) {
        if let Some((channel, control, value)) = control_value(msg) {
            self.store_cc(channel, control, value);
        } else if let Some((channel, bend)) = bend_value(msg) {
            self.store_bend(channel, bend);
        }
    }

    fn admit_cc(&mut self, channel: u8, control: u8, value: u8) -> bool {
        if self.lookup_cc(channel, control) == Some(value) {
            return false;
        }
        self.store_cc(channel, control, value);
        true
    }

    fn admit_bend(&mut self, channel: u8, bend: u16) -> bool {
        let slot = usize::from(channel);
        if self.bend.get(slot) == Some(&Some(bend)) {
            return false;
        }
        self.store_bend(channel, bend);
        true
    }

    fn lookup_cc(&self, channel: u8, control: u8) -> Option<u8> {
        self.cc
            .iter()
            .flatten()
            .find(|entry| entry.channel == channel && entry.control == control)
            .map(|entry| entry.value)
    }

    fn store_cc(&mut self, channel: u8, control: u8, value: u8) {
        if let Some(entry) = self
            .cc
            .iter_mut()
            .flatten()
            .find(|entry| entry.channel == channel && entry.control == control)
        {
            entry.value = value;
            return;
        }
        let slot = if let Some(slot) = self.cc.iter().position(Option::is_none) {
            slot
        } else {
            // Full: replace the first entry. Controllers in use are few, so a
            // simple eviction is enough and keeps the path branch-free-ish.
            0
        };
        self.cc[slot] = Some(CcEntry {
            channel,
            control,
            value,
        });
    }

    fn store_bend(&mut self, channel: u8, bend: u16) {
        if let Some(slot) = self.bend.get_mut(usize::from(channel)) {
            *slot = Some(bend);
        }
    }
}

impl Default for MidiOutputCache {
    fn default() -> Self {
        Self::new()
    }
}

fn control_value(msg: &MidiMsg) -> Option<(u8, u8, u8)> {
    let MidiMsg::ChannelVoice { channel, msg } = msg else {
        return None;
    };
    let ChannelVoiceMsg::ControlChange {
        control: ControlChange::CC { control, value },
    } = msg
    else {
        return None;
    };
    Some((*channel as u8, *control, *value))
}

fn bend_value(msg: &MidiMsg) -> Option<(u8, u16)> {
    let MidiMsg::ChannelVoice { channel, msg } = msg else {
        return None;
    };
    let ChannelVoiceMsg::PitchBend { bend } = msg else {
        return None;
    };
    Some((*channel as u8, *bend))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{VoiceId, VoiceOutput},
        midi::MidiController,
    };

    fn cc(channel: u8, control: u8, value: u8) -> MidiMsg {
        MidiMsg::ChannelVoice {
            channel: midi_msg::Channel::from_u8(channel),
            msg: ChannelVoiceMsg::ControlChange {
                control: ControlChange::CC { control, value },
            },
        }
    }

    fn bend(channel: u8, bend: u16) -> MidiMsg {
        MidiMsg::ChannelVoice {
            channel: midi_msg::Channel::from_u8(channel),
            msg: ChannelVoiceMsg::PitchBend { bend },
        }
    }

    fn control_output(value: f32) -> MidiMsg {
        MidiController::new()
            .encode(VoiceOutput::Control {
                voice: VoiceId(0),
                slot: 0,
                value,
            })
            .expect("encodes")
    }

    #[test]
    fn repeated_cc_is_suppressed() {
        let mut cache = MidiOutputCache::new();
        assert!(cache.admit_semantic(&cc(0, 3, 40)));
        assert!(!cache.admit_semantic(&cc(0, 3, 40)));
        assert!(cache.admit_semantic(&cc(0, 3, 41)), "a change passes");
    }

    #[test]
    fn floats_that_quantise_alike_send_once() {
        // Both round to 40 at 7-bit resolution, which the animator cannot see.
        let first = control_output(0.3141);
        let second = control_output(0.3142);
        assert_eq!(first, second, "same wire message");

        let mut cache = MidiOutputCache::new();
        assert!(cache.admit_semantic(&first));
        assert!(!cache.admit_semantic(&second), "duplicate suppressed");
    }

    #[test]
    fn different_controllers_are_independent() {
        let mut cache = MidiOutputCache::new();
        assert!(cache.admit_semantic(&cc(0, 3, 40)));
        assert!(cache.admit_semantic(&cc(0, 4, 40)), "different control");
    }

    #[test]
    fn channels_are_independent() {
        let mut cache = MidiOutputCache::new();
        assert!(cache.admit_semantic(&cc(0, 3, 40)));
        assert!(cache.admit_semantic(&cc(1, 3, 40)), "different channel");
    }

    #[test]
    fn pitch_bend_is_per_channel() {
        let mut cache = MidiOutputCache::new();
        assert!(cache.admit_semantic(&bend(0, 9000)));
        assert!(!cache.admit_semantic(&bend(0, 9000)), "duplicate");
        assert!(cache.admit_semantic(&bend(1, 9000)), "other channel");
        assert!(cache.admit_semantic(&bend(0, 9001)), "value changed");
    }

    #[test]
    fn notes_are_never_suppressed() {
        let mut cache = MidiOutputCache::new();
        let note = MidiMsg::ChannelVoice {
            channel: midi_msg::Channel::Ch1,
            msg: ChannelVoiceMsg::NoteOn {
                note: 60,
                velocity: 100,
            },
        };
        assert!(cache.admit_semantic(&note));
        assert!(cache.admit_semantic(&note), "repeated notes still send");
    }

    #[test]
    fn raw_cc_teaches_the_cache() {
        let mut cache = MidiOutputCache::new();
        cache.record_raw(&cc(0, 3, 40));
        assert!(
            !cache.admit_semantic(&cc(0, 3, 40)),
            "wire already holds 40"
        );
        assert!(
            cache.admit_semantic(&cc(0, 3, 41)),
            "wire differs, so send"
        );
    }

    #[test]
    fn raw_cc_that_repeats_is_not_recorded_as_suppression() {
        // record_raw never reports suppression; it only updates state.
        let mut cache = MidiOutputCache::new();
        cache.record_raw(&cc(0, 3, 40));
        cache.record_raw(&cc(0, 3, 40));
        assert!(!cache.admit_semantic(&cc(0, 3, 40)));
    }

    #[test]
    fn many_controllers_fit_without_losing_recent_ones() {
        let mut cache = MidiOutputCache::new();
        for control in 0..8u8 {
            assert!(cache.admit_semantic(&cc(0, control, 10 + control)));
        }
        for control in 0..8u8 {
            assert!(
                !cache.admit_semantic(&cc(0, control, 10 + control)),
                "control {control} still remembered"
            );
        }
    }
}
