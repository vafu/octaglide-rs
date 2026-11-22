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
                    idx: index,
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
                if let Some(target) = self.voice_mut(voice) {
                    let outputs = target.process_input(input).await;
                    self.emit(outputs).await;
                }
            }
            Input::ControlUpdate { voice, lane, .. } | Input::ModeUpdate { voice, lane, .. } => {
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
            .map(Consumers::snapshot)
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
                    .position(|consumer| matches!(consumer, Consumers::ModTrigger(_)))
            })
    }

    fn voice(&self, id: VoiceId) -> Option<&Voice> {
        let idx = id.0 as usize;
        let voice = self.voices.get(idx)?;
        debug_assert_eq!(voice.idx, idx, "voice index must match its position");
        Some(voice)
    }

    fn voice_mut(&mut self, id: VoiceId) -> Option<&mut Voice> {
        let idx = id.0 as usize;
        let voice = self.voices.get_mut(idx)?;
        debug_assert_eq!(voice.idx, idx, "voice index must match its position");
        Some(voice)
    }

    async fn emit(&mut self, messages: impl IntoIterator<Item = VoiceOutput>) {
        for msg in messages {
            self.output.send(OutputMessage::Semantic(msg)).await.ok();
        }
    }
}
