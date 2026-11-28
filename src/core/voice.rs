use super::{
    Consumer, Input, Modulation, NoteKind, NoteTransition, OutputEvents, Routing,
    SlotConsumer, Voice, VoiceOutput,
};

impl Voice {
    /// Fans one note out to every consumer that wants notes.
    ///
    /// A consumer that consumes the note and emits a replacement (voicing)
    /// changes what later slots see, so the transpose applies downstream.
    pub(super) fn process_note(&mut self, transition: NoteTransition) -> OutputEvents {
        let mut out = OutputEvents::new();
        let mut current = transition;

        for consumer in self.slots.iter_mut() {
            if !consumer.wants_notes() {
                continue;
            }
            let mut produced = OutputEvents::new();
            let routing = consumer.consume(&Input::Note(current), &mut produced);

            if routing == Routing::Consumed && !produced.is_empty() {
                if let Some(next) = retarget(&produced, current) {
                    current = next;
                }
            }
            for msg in produced {
                out.push(msg).ok();
            }
        }

        out
    }

    /// Runs one addressed event through exactly one slot.
    pub(super) fn process_addressed(&mut self, slot: u8, input: Input) -> OutputEvents {
        let Some(consumer) = self.slots.get_mut(slot as usize) else {
            return OutputEvents::new();
        };
        let mut out = OutputEvents::new();
        consumer.consume(&input, &mut out);
        out
    }

    pub(super) fn advance(&mut self, delta_ms: u32) -> OutputEvents {
        let gated = self.notes.any_held();
        let mut out = OutputEvents::new();
        for consumer in self.slots.iter_mut() {
            if let Some(messages) = consumer.advance(delta_ms, gated) {
                for msg in messages {
                    out.push(msg).ok();
                }
            }
        }
        out
    }

    pub(super) fn snapshot(&self, slot: u8) -> Option<super::ConsumerSnapshot> {
        Some(self.slots.get(slot as usize)?.snapshot())
    }
}

fn retarget(produced: &OutputEvents, current: NoteTransition) -> Option<NoteTransition> {
    produced.iter().find_map(|msg| match *msg {
        VoiceOutput::NoteOn { note, velocity, .. } if current.kind == NoteKind::Press => {
            Some(NoteTransition { note, velocity, ..current })
        }
        VoiceOutput::NoteOff { note, velocity, .. } if current.kind == NoteKind::Release => {
            Some(NoteTransition { note, velocity, ..current })
        }
        _ => None,
    })
}

impl SlotConsumer {
    pub(super) fn snapshot(&self) -> super::ConsumerSnapshot {
        let mut controls = [None; super::CONTROL_SLOTS];
        for (slot, value) in controls.iter_mut().enumerate() {
            *value = self.control_slot_value(slot as u8);
        }
        super::ConsumerSnapshot {
            kind: self.kind(),
            output_slot: self.output_slot(),
            controls,
            mode: self.mode_index(),
            mode_count: self.mode_count(),
        }
    }

    pub(super) fn kind(&self) -> super::ConsumerKind {
        match self {
            Self::Noop(_) => super::ConsumerKind::Noop,
            Self::Octave(_) => super::ConsumerKind::Octave,
            Self::Glider(_) => super::ConsumerKind::Glider,
            Self::ModTrigger(_) => super::ConsumerKind::ModTrigger,
        }
    }
}
