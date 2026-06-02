use crate::{
    app::MidiSender,
    core::{
        MidiOut,
        event::{Event, EventPayload, EventRole, Events},
    },
    state,
};

#[derive(Debug)]
pub struct Passthrough {
    midi_sender: MidiSender,
}

impl Passthrough {
    pub fn new(midi_sender: MidiSender) -> Self {
        Self { midi_sender }
    }
}

impl super::Consumer for Passthrough {
    async fn consume(&mut self, event: Event, _out: &mut Events) {
        let EventRole::Voice(voice) = event.role else {
            return;
        };

        let Some(channel) = state::read_voice(voice, |voice| voice.output_channel).await else {
            return;
        };

        let msg = match event.payload {
            EventPayload::NoteOn { note, velocity } => midi_msg::MidiMsg::ChannelVoice {
                channel,
                msg: midi_msg::ChannelVoiceMsg::NoteOn { note, velocity },
            },
            EventPayload::NoteOff { note, velocity } => midi_msg::MidiMsg::ChannelVoice {
                channel,
                msg: midi_msg::ChannelVoiceMsg::NoteOff { note, velocity },
            },
            EventPayload::Control(_) | EventPayload::Delta(_) => return,
        };

        self.midi_sender
            .send(MidiOut { msg, tag: "pass" })
            .await
            .ok();
    }
}
