use heapless::Vec;

use crate::output::{OutputMessage, OutputSender};

use super::consumers::{Glider, ModTrigger, Octave, Passthrough};
use super::{
    Consumers, Core, Input, MAX_VOICES, Voice, VoiceAnimators, VoiceId, VoiceOutput,
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
                    id,
                    voicing: vec![
                        Consumers::Octave(Octave::new(id)),
                        Consumers::Glider(Glider::new(id, resources.glide)),
                    ],
                    modulations: [
                        Consumers::Passthrough(Passthrough),
                        Consumers::Passthrough(Passthrough),
                        Consumers::ModTrigger(ModTrigger::new(id, 0, first_envelope)),
                        Consumers::ModTrigger(ModTrigger::new(id, 1, second_envelope)),
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
                if let Some(target) = self.voices.iter_mut().find(|v| v.id == voice) {
                    let outputs = target.process_input(input).await;
                    self.emit(outputs).await;
                }
            }
            Input::ControlUpdate { voice, lane, .. } | Input::ModeUpdate { voice, lane, .. } => {
                if let Some(target) = self.voices.iter_mut().find(|v| v.id == voice) {
                    let outputs = target.process_lane(lane, input).await;
                    self.emit(outputs).await;
                }
            }
        }
    }

    pub fn snapshot(&self, voice: VoiceId, lane: usize) -> Option<super::ConsumerSnapshot> {
        self.voices
            .iter()
            .find(|candidate| candidate.id == voice)
            .and_then(|voice| voice.modulations.get(lane))
            .map(Consumers::snapshot)
    }

    pub fn lane_count(&self, voice: VoiceId) -> Option<usize> {
        self.voices
            .iter()
            .find(|candidate| candidate.id == voice)
            .map(|voice| voice.modulations.len())
    }

    pub fn first_envelope_lane(&self, voice: VoiceId) -> Option<usize> {
        self.voices
            .iter()
            .find(|candidate| candidate.id == voice)
            .and_then(|voice| {
                voice
                    .modulations
                    .iter()
                    .position(|consumer| matches!(consumer, Consumers::ModTrigger(_)))
            })
    }

    async fn emit(&mut self, messages: impl IntoIterator<Item = VoiceOutput>) {
        for msg in messages {
            self.output.send(OutputMessage::Semantic(msg)).await.ok();
        }
    }
}
