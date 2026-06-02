use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::state::{self, ChannelRoute};

use super::event::{Event, EventPayload, EventRole, normalize_midi_cc};

#[derive(Clone, Debug, PartialEq)]
pub enum RoutedMidi {
    Event(Event),
    Passthrough(MidiMsg),
    Drop,
}

pub async fn route_midi(msg: MidiMsg) -> RoutedMidi {
    let Some((channel, payload)) = parse_channel_payload(&msg) else {
        return RoutedMidi::Passthrough(msg);
    };

    match state::route_for_channel(channel).await {
        ChannelRoute::Off => RoutedMidi::Drop,
        ChannelRoute::External => RoutedMidi::Passthrough(msg),
        ChannelRoute::Voice(voice) => match payload {
            EventPayload::NoteOn { .. } | EventPayload::NoteOff { .. } => {
                RoutedMidi::Event(Event {
                    role: EventRole::Voice(voice),
                    payload,
                })
            }
            EventPayload::Control(_) | EventPayload::Delta(_) => RoutedMidi::Passthrough(msg),
        },
        ChannelRoute::Modifier { voice, param } => match payload {
            EventPayload::Control(_) | EventPayload::Delta(_) => RoutedMidi::Event(Event {
                role: EventRole::Modifier { voice, param },
                payload,
            }),
            EventPayload::NoteOn { .. } | EventPayload::NoteOff { .. } => RoutedMidi::Drop,
        },
    }
}

fn parse_channel_payload(msg: &MidiMsg) -> Option<(Channel, EventPayload)> {
    let (MidiMsg::ChannelVoice { channel, msg } | MidiMsg::RunningChannelVoice { channel, msg }) =
        msg
    else {
        return None;
    };

    let payload = match msg {
        ChannelVoiceMsg::NoteOn { note, velocity } if *velocity == 0 => EventPayload::NoteOff {
            note: *note,
            velocity: 0,
        },
        ChannelVoiceMsg::NoteOn { note, velocity } => EventPayload::NoteOn {
            note: *note,
            velocity: *velocity,
        },
        ChannelVoiceMsg::NoteOff { note, velocity } => EventPayload::NoteOff {
            note: *note,
            velocity: *velocity,
        },
        ChannelVoiceMsg::ControlChange {
            control: ControlChange::CC { value, .. },
        } => EventPayload::Control(normalize_midi_cc(*value)),
        _ => return None,
    };

    Some((*channel, payload))
}
