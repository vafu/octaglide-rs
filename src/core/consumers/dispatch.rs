use super::{Consumer, ConsumerKind, ModLane, ModLanes};
use crate::core::{ConsumerSnapshot, CONTROL_SLOTS};

impl ModLanes {
    pub(crate) fn kind(&self) -> ConsumerKind {
        match self {
            Self::Passthrough(_) => ConsumerKind::Passthrough,
            Self::ModTrigger(_) => ConsumerKind::ModTrigger,
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
            mode_count: self.mode_count(),
        }
    }
}
