use heapless::Vec;
use midi_msg::{Channel, ChannelVoiceMsg, MidiMsg};

use crate::{
    anim::VoiceAnimators,
    app::MidiSender,
    midi::log_midi,
    ui::{UiConsumer, UiWriter},
};

use super::{
    Consumer, Core, Input, MAX_VOICES, MidiOut, OutputEvents, Track, TrackId, Voice, VoiceId,
    consumers::{Consumers, Glider, ModTrigger, Octave, SliderCatchUp},
};

macro_rules! vec {
    ($($item:expr),* $(,)?) => {{
        let mut v = Vec::new();
        $(v.push($item).unwrap();)*
        v
    }};
}

impl Core {
    pub fn new(
        midi_sender: MidiSender,
        animators: [VoiceAnimators; MAX_VOICES],
        ui_writer: UiWriter,
    ) -> Self {
        let channels = [Channel::Ch1, Channel::Ch2, Channel::Ch3, Channel::Ch4];
        let auxiliary = [Channel::Ch5, Channel::Ch6, Channel::Ch7, Channel::Ch8];
        let mut tracks = Vec::new();
        let mut voices = Vec::new();
        for (index, resources) in animators.into_iter().enumerate() {
            let id = VoiceId(index as u8);
            let output = channels[index];
            let [first_envelope, second_envelope] = resources.envelopes;
            voices
                .push(Voice {
                    id,
                    output_channel: output,
                    midi_sender: midi_sender.clone(),
                    voicing: vec![
                        Consumers::Octave(Octave::new()),
                        Consumers::Glider(Glider::new(id, output, resources.glide)),
                    ],
                    modulations: [
                        Consumers::Passthrough,
                        Consumers::Passthrough,
                        Consumers::ModTrigger(SliderCatchUp::new(ModTrigger::new(
                            id,
                            output,
                            2,
                            first_envelope,
                        ))),
                        Consumers::ModTrigger(SliderCatchUp::new(ModTrigger::new(
                            id,
                            output,
                            3,
                            second_envelope,
                        ))),
                    ],
                })
                .unwrap();
            tracks
                .push(Track {
                    id: TrackId(index as u8),
                    main_channel: channels[index],
                    aux_channel: Some(auxiliary[index]),
                    voices: vec![id],
                })
                .unwrap();
        }
        let mut core = Core {
            ui_consumer: UiConsumer::new(),
            tracks,
            voices,
            midi_sender,
            selected_track: TrackId(0),
            selected_voice: VoiceId(0),
            selected_lane: 2, // First envelope, modulation lane 3.
            ui_writer,
            ui_dirty: true,
            usb_connected: false,
        };
        core.publish_ui();
        core
    }

    pub async fn process(&mut self, input: Input) {
        let mut generated = OutputEvents::new();
        let consumed = self.ui_consumer.consume(&input, &mut generated).await;
        if let Some(track) = self.ui_consumer.take_track_selection() {
            self.select_track(track);
        }
        if let Some(lane) = self.ui_consumer.take_lane_selection() {
            self.select(self.selected_track, self.selected_voice, lane);
        }
        self.sync_slider_pickup(false);
        for msg in generated {
            self.process_midi(msg).await;
        }
        if consumed {
            self.publish_ui();
            return;
        }
        match input {
            Input::Midi(msg) => self.process_midi(msg).await,
            Input::Select { track, voice, lane } => {
                self.select(track, voice, lane);
            }
            Input::UsbConnection { connected } => {
                self.ui_dirty |= self.usb_connected != connected;
                self.usb_connected = connected;
            }
            hardware => self.process_hardware(hardware).await,
        }
        self.publish_ui();
    }

    async fn process_hardware(&mut self, mut input: Input) {
        self.ui_consumer.map_hardware_input(&mut input);
        let Some(voice) = self
            .voices
            .iter_mut()
            .find(|voice| voice.id == self.selected_voice)
        else {
            return;
        };
        // Only the selected consumer sees the hardware action. Any MIDI it
        // generates continues through the remaining consumers in that voice.
        voice.process_lane(self.selected_lane, input).await;
    }

    pub(super) fn sync_slider_pickup(&mut self, rearm: bool) {
        let (positions, alternate) = self.ui_consumer.slider_context();
        if let Some(consumer) = self
            .voices
            .iter_mut()
            .find(|voice| voice.id == self.selected_voice)
            .and_then(|voice| voice.modulations.get_mut(self.selected_lane))
        {
            consumer.set_slider_context(positions, alternate, rearm);
        }
    }

    /// Resolve MIDI against this Core's tracks and run their selected voices.
    async fn process_midi(&mut self, mut msg: MidiMsg) {
        let channel = match &msg {
            MidiMsg::ChannelVoice { channel, .. }
            | MidiMsg::RunningChannelVoice { channel, .. }
            | MidiMsg::ChannelMode { channel, .. }
            | MidiMsg::RunningChannelMode { channel, .. } => *channel,
            _ => return self.send_raw(msg, "raw").await,
        };
        let Some(track) = self
            .tracks
            .iter()
            .find(|track| track.main_channel == channel || track.aux_channel == Some(channel))
        else {
            return self.send_raw(msg, "raw").await;
        };
        // Normalize once before any voice or consumer sees the message.
        if let MidiMsg::ChannelVoice { msg: voice_msg, .. }
        | MidiMsg::RunningChannelVoice { msg: voice_msg, .. } = &mut msg
            && let ChannelVoiceMsg::NoteOn { note, velocity: 0 } = *voice_msg
        {
            *voice_msg = ChannelVoiceMsg::NoteOff { note, velocity: 0 };
        }
        for &voice_id in &track.voices {
            if let Some(voice) = self.voices.iter_mut().find(|voice| voice.id == voice_id) {
                voice.process_midi(msg.clone()).await;
            }
        }
    }

    async fn send_raw(&mut self, msg: MidiMsg, tag: &'static str) {
        log_midi("<<<", None, &msg);
        self.midi_sender.send(MidiOut { msg, tag }).await.ok();
    }
}
