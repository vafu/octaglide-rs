use super::{
    Consumer, Input, Modulation, NoteKind, NoteTransition, OutputEvents, Routing,
    SlotConsumer, Voice, VoiceOutput,
};

impl Voice {
    /// Fans one note out to every consumer that wants notes.
    ///
    /// Voicing output replaces the note for the rest of the chain, so a later
    /// section sees the transposed note rather than the original.
    pub(super) fn process_note(&mut self, transition: NoteTransition) -> OutputEvents {
        let mut out = OutputEvents::new();
        let mut current = transition;
        let mut replaced = false;

        for section in self.slots.iter_mut() {
            for consumer in section.iter_mut() {
                if !consumer.wants_notes() {
                    continue;
                }
                let mut produced = OutputEvents::new();
                let routing = consumer.consume(&Input::Note(current), &mut produced);

                if routing == Routing::Consumed && !produced.is_empty() {
                    // Voicing output becomes the note the next section sees.
                    if let Some(next) = retarget(&produced, current) {
                        current = next;
                        replaced = true;
                    }
                }
                for msg in produced {
                    out.push(msg).ok();
                }
            }
        }

        let _ = replaced;
        out
    }

    /// Runs one addressed event through exactly one slot.
    pub(super) fn process_addressed(&mut self, slot: super::Slot, input: Input) -> OutputEvents {
        let Some(consumer) = self
            .slots
            .get_mut(slot.section.index() as usize)
            .and_then(|section| section.get_mut(slot.index as usize))
        else {
            return OutputEvents::new();
        };
        let mut out = OutputEvents::new();
        consumer.consume(&input, &mut out);
        out
    }

    pub(super) fn advance(&mut self, delta_ms: u32) -> OutputEvents {
        let gated = self.notes.any_held();
        let mut out = OutputEvents::new();
        for section in self.slots.iter_mut() {
            for consumer in section.iter_mut() {
                if let Some(messages) = consumer.advance(delta_ms, gated) {
                    for msg in messages {
                        out.push(msg).ok();
                    }
                }
            }
        }
        out
    }

    pub(super) fn snapshot(&self, slot: super::Slot) -> Option<super::ConsumerSnapshot> {
        let consumer = self
            .slots
            .get(slot.section.index() as usize)
            .and_then(|section| section.get(slot.index as usize))?;
        Some(consumer.snapshot())
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
