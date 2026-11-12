use super::{Consumer, ConsumerKind, Consumers, Glider, ModTrigger, Octave};
use crate::core::{ConsumerSnapshot, Input, OutputEvents, CONTROL_SLOTS};

impl Consumers {
    pub(crate) fn kind(&self) -> ConsumerKind {
        match self {
            Self::Passthrough => ConsumerKind::Passthrough,
            Self::Octave(_) => ConsumerKind::Octave,
            Self::ModTrigger(_) => ConsumerKind::ModTrigger,
            Self::Glider(_) => ConsumerKind::Glider,
        }
    }

    pub(crate) fn set_control_slot(&mut self, slot: u8, value: f32) {
        match self {
            Self::Passthrough => {}
            Self::Octave(consumer) => {
                if let Some(control) = <Octave as Consumer>::CONTROL_MAP
                    .get(slot as usize)
                    .copied()
                    .flatten()
                {
                    consumer.set_control(control, value);
                }
            }
            Self::ModTrigger(consumer) => {
                if let Some(control) = <ModTrigger as Consumer>::CONTROL_MAP
                    .get(slot as usize)
                    .copied()
                    .flatten()
                {
                    consumer.set_control(control, value);
                }
            }
            Self::Glider(consumer) => {
                if let Some(control) = <Glider as Consumer>::CONTROL_MAP
                    .get(slot as usize)
                    .copied()
                    .flatten()
                {
                    consumer.set_control(control, value);
                }
            }
        }
    }

    pub(crate) fn control_slot_value(&self, slot: u8) -> Option<f32> {
        match self {
            Self::Passthrough => None,
            Self::Octave(consumer) => <Octave as Consumer>::CONTROL_MAP
                .get(slot as usize)
                .copied()
                .flatten()
                .map(|control| consumer.control_value(control)),
            Self::ModTrigger(consumer) => <ModTrigger as Consumer>::CONTROL_MAP
                .get(slot as usize)
                .copied()
                .flatten()
                .map(|control| consumer.control_value(control)),
            Self::Glider(consumer) => <Glider as Consumer>::CONTROL_MAP
                .get(slot as usize)
                .copied()
                .flatten()
                .map(|control| consumer.control_value(control)),
        }
    }

    pub(crate) fn update_mode(&mut self, direction: i8) {
        match self {
            Self::Passthrough => {}
            Self::Octave(consumer) => consumer.update_mode(direction),
            Self::ModTrigger(consumer) => consumer.update_mode(direction),
            Self::Glider(consumer) => consumer.update_mode(direction),
        }
    }

    pub(crate) fn mode_index(&self) -> u8 {
        match self {
            Self::Passthrough => 0,
            Self::Octave(consumer) => consumer.mode().index(),
            Self::ModTrigger(consumer) => consumer.mode().index(),
            Self::Glider(consumer) => consumer.mode().index(),
        }
    }

    pub(crate) fn snapshot(&self) -> ConsumerSnapshot {
        let mut controls = [None; CONTROL_SLOTS];
        for (slot, value) in controls.iter_mut().enumerate() {
            *value = self.control_slot_value(slot as u8);
        }
        ConsumerSnapshot {
            kind: self.kind(),
            output_slot: match self {
                Self::ModTrigger(consumer) => consumer.output_slot(),
                _ => 0,
            },
            controls,
            mode: self.mode_index(),
        }
    }
}

impl Consumer for Consumers {
    type Control = u8;
    type Mode = u8;

    const CONTROL_MAP: [Option<Self::Control>; CONTROL_SLOTS] = [None; CONTROL_SLOTS];
    const MODE_COUNT: u8 = 1;

    async fn consume(&mut self, input: &Input, out: &mut OutputEvents) -> bool {
        match self {
            Self::Passthrough => false,
            Self::Octave(consumer) => consumer.consume(input, out).await,
            Self::ModTrigger(consumer) => consumer.consume(input, out).await,
            Self::Glider(consumer) => consumer.consume(input, out).await,
        }
    }

    fn set_control(&mut self, _control: Self::Control, _value: f32) {}
    fn control_value(&self, _control: Self::Control) -> f32 { 0.0 }
    fn mode(&self) -> Self::Mode { 0 }
    fn update_mode(&mut self, _direction: i8) {}
}
