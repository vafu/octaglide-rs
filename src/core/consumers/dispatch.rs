use super::{Consumer, ConsumerKind, Consumers};
use crate::core::{ConsumerSnapshot, CONTROL_SLOTS};

impl Consumers {
    pub(crate) fn kind(&self) -> ConsumerKind {
        match self {
            Self::Passthrough(_) => ConsumerKind::Passthrough,
            Self::Octave(_) => ConsumerKind::Octave,
            Self::ModTrigger(_) => ConsumerKind::ModTrigger,
            Self::Glider(_) => ConsumerKind::Glider,
        }
    }

    pub(crate) fn snapshot(&self) -> ConsumerSnapshot {
        let mut controls = [None; CONTROL_SLOTS];
        for (slot, value) in controls.iter_mut().enumerate() {
            *value = self.control_slot_value(slot as u8);
        }
        ConsumerSnapshot {
            kind: self.kind(),
            output_slot: self.output_slot(),
            controls,
            mode: self.mode_index(),
        }
    }
}
