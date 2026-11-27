use heapless::Vec;

use crate::output::{OutputMessage, OutputSender};

use super::consumers::{Glider, ModTrigger, Noop, Octave, SlotConsumer};
use super::{
    Core, HeldNotes, Input, NoteKind, NoteTransition, OutputEvents, Section, Selection, Slot,
    Voice, VoiceId, MAX_VOICES, SLOTS_PER_SECTION,
};

impl Core {
    pub fn new(output: OutputSender) -> Self {
        let mut voices = Vec::new();
        for index in 0..MAX_VOICES {
            let id = VoiceId(index as u8);
            let noop = || SlotConsumer::Noop(Noop);
            voices
                .push(Voice {
                    slots: [
                        [noop(), noop(), noop(), noop()],
                        [
                            SlotConsumer::Octave(Octave::new(id)),
                            SlotConsumer::Glider(Glider::new(id)),
                            noop(),
                            noop(),
                        ],
                        [
                            SlotConsumer::ModTrigger(ModTrigger::new(id, 0)),
                            SlotConsumer::ModTrigger(ModTrigger::new(id, 1)),
                            noop(),
                            noop(),
                        ],
                        [noop(), noop(), noop(), noop()],
                    ],
                    notes: HeldNotes::new(),
                })
                .unwrap();
        }
        Core { voices, output }
    }

    pub async fn process(&mut self, input: Input) {
        match input {
            Input::Note(transition) => {
                let voice = transition.voice;
                let Some(target) = self.voice_mut(voice) else {
                    return;
                };
                let transition = update_notes(&mut target.notes, transition);
                let outputs = target.process_note(transition);
                self.emit(outputs).await;
            }
            Input::ControlUpdate { voice, slot, .. } | Input::ModeSet { voice, slot, .. } => {
                if let Some(target) = self.voice_mut(voice) {
                    let outputs = target.process_addressed(slot, input);
                    self.emit(outputs).await;
                }
            }
        }
    }

    /// Advances every timed consumer by one tick.
    pub async fn advance(&mut self, delta_ms: u32) {
        let mut out = OutputEvents::new();
        for voice in self.voices.iter_mut() {
            for msg in voice.advance(delta_ms) {
                out.push(msg).ok();
            }
        }
        self.emit(out).await;
    }

    /// Stops every running modulator and emits its release.
    pub async fn reset_modulators(&mut self) {
        let mut out = OutputEvents::new();
        for voice in self.voices.iter_mut() {
            for msg in voice.reset_modulators() {
                out.push(msg).ok();
            }
        }
        self.emit(out).await;
    }

    pub fn snapshot(&self, voice: VoiceId, slot: Slot) -> Option<super::ConsumerSnapshot> {
        self.voice(voice).and_then(|voice| voice.snapshot(slot))
    }

    pub fn section_snapshot(
        &self,
        voice: VoiceId,
        section: Section,
    ) -> [Option<super::ConsumerSnapshot>; SLOTS_PER_SECTION] {
        core::array::from_fn(|index| self.snapshot(voice, Slot::new(section, index as u8)))
    }

    /// First slot in a section holding a given consumer kind.
    pub fn find(&self, voice: VoiceId, kind: super::ConsumerKind) -> Option<Slot> {
        let voice = self.voice(voice)?;
        for (section_index, section) in voice.slots.iter().enumerate() {
            for (index, consumer) in section.iter().enumerate() {
                if consumer.kind() == kind {
                    return Some(Slot::new(
                        Section::from_index(section_index as u8),
                        index as u8,
                    ));
                }
            }
        }
        None
    }

    fn voice(&self, id: VoiceId) -> Option<&Voice> {
        self.voices.get(id.0 as usize)
    }

    fn voice_mut(&mut self, id: VoiceId) -> Option<&mut Voice> {
        self.voices.get_mut(id.0 as usize)
    }

    async fn emit(&mut self, messages: impl IntoIterator<Item = super::VoiceOutput>) {
        for msg in messages {
            self.output.send(OutputMessage::Semantic(msg)).await.ok();
        }
    }
}

fn update_notes(notes: &mut HeldNotes, transition: NoteTransition) -> NoteTransition {
    match transition.kind {
        NoteKind::Press => {
            let result = notes.press(transition.note);
            NoteTransition {
                previous_top: result.previous_top,
                was_top: false,
                new_top: None,
                any_held: notes.any_held(),
                ..transition
            }
        }
        NoteKind::Release => {
            let result = notes.release(transition.note);
            NoteTransition {
                previous_top: None,
                was_top: result.is_some_and(|r| r.was_top),
                new_top: result.and_then(|r| r.new_top),
                any_held: notes.any_held(),
                ..transition
            }
        }
    }
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            voice: VoiceId(0),
            slot: Slot::new(Section::Modulation, 0),
        }
    }
}
