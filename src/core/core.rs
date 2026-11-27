use heapless::Vec;

use crate::output::{OutputMessage, OutputSender};

use super::consumers::{Glider, ModTrigger, Octave, Passthrough};
use super::{
    Core, Input, ModLanes, Voice, VoiceAnimators, VoiceId, VoiceOutput, Voicing, MAX_VOICES,
};

macro_rules! vec {
    ($($item:expr),* $(,)?) => {{
        let mut v = Vec::new();
        $(v.push($item).unwrap();)*
        v
    }};
}

impl Core {
    pub fn new(output: OutputSender, animators: [VoiceAnimators; MAX_VOICES]) -> Self {
        let mut voices = Vec::new();
        for (index, resources) in animators.into_iter().enumerate() {
            let id = VoiceId(index as u8);
            let [first_envelope, second_envelope] = resources.envelopes;
            voices
                .push(Voice {
                    voicing: vec![
                        Voicing::Octave(Octave::new(id)),
                        Voicing::Glider(Glider::new(id, resources.glide[0].clone())),
                    ],
                    modulations: [
                        ModLanes::Passthrough(Passthrough),
                        ModLanes::Passthrough(Passthrough),
                        ModLanes::ModTrigger(ModTrigger::new(id, 0, first_envelope.clone())),
                        ModLanes::ModTrigger(ModTrigger::new(id, 1, second_envelope)),
                    ],
                })
                .unwrap();
        }
        Core {
            voices,
            output,
        }
    }

    pub async fn process(&mut self, input: Input) {
        match input {
            Input::NoteOn { voice, .. } | Input::NoteOff { voice, .. } => {
                if let Some(target) = self.voice_mut(voice) {
                    let outputs = target.process_input(input).await;
                    self.emit(outputs).await;
                }
            }
            Input::ControlUpdate { voice, lane, .. } | Input::ModeSet { voice, lane, .. } => {
                if let Some(target) = self.voice_mut(voice) {
                    let outputs = target.process_lane(lane, input).await;
                    self.emit(outputs).await;
                }
            }
        }
    }

    pub fn snapshot(&self, voice: VoiceId, lane: usize) -> Option<super::ConsumerSnapshot> {
        self.voice(voice)
            .and_then(|voice| voice.modulations.get(lane))
            .map(ModLanes::snapshot)
    }

    pub fn lane_count(&self, voice: VoiceId) -> Option<usize> {
        self.voice(voice)
            .map(|voice| voice.modulations.len())
    }

    pub fn first_envelope_lane(&self, voice: VoiceId) -> Option<usize> {
        self.voice(voice)
            .and_then(|voice| {
                voice
                    .modulations
                    .iter()
                    .position(|consumer| matches!(consumer, ModLanes::ModTrigger(_)))
            })
    }

    fn voice(&self, id: VoiceId) -> Option<&Voice> {
        self.voices.get(id.0 as usize)
    }

    fn voice_mut(&mut self, id: VoiceId) -> Option<&mut Voice> {
        self.voices.get_mut(id.0 as usize)
    }

    async fn emit(&mut self, messages: impl IntoIterator<Item = VoiceOutput>) {
        for msg in messages {
            self.output.send(OutputMessage::Semantic(msg)).await.ok();
        }
    }
}
