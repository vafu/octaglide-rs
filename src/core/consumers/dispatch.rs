use super::{Consumer, Consumers};
use crate::{
    core::{Input, OutputEvents},
    ui::{UiModel, UiProvider},
};

impl Consumers {
    pub(in crate::core) fn set_slider_context(
        &mut self,
        positions: [Option<u16>; 4],
        alternate: bool,
        rearm: bool,
    ) {
        if let Self::ModTrigger(consumer) = self {
            consumer.set_slider_context(positions, alternate, rearm);
        }
    }
}

impl Consumer for Consumers {
    async fn consume(&mut self, input: &Input, out: &mut OutputEvents) -> bool {
        match self {
            Self::Passthrough => false,
            Self::Octave(consumer) => consumer.consume(input, out).await,
            Self::ModTrigger(consumer) => consumer.consume(input, out).await,
            Self::Glider(consumer) => consumer.consume(input, out).await,
        }
    }
}

// The enum adapts each variant to its provider model.
impl UiProvider for Consumers {
    type Model = Option<UiModel>;

    fn ui_snapshot(&self) -> Self::Model {
        match self {
            Self::Passthrough => None,
            Self::Octave(consumer) => Some(UiModel::Octave(consumer.ui_snapshot())),
            Self::ModTrigger(consumer) => Some(UiModel::ModTrigger(consumer.ui_snapshot())),
            Self::Glider(consumer) => Some(UiModel::Glider(consumer.ui_snapshot())),
        }
    }

    fn has_updates(&mut self) -> bool {
        match self {
            Self::Passthrough => false,
            Self::Octave(consumer) => consumer.has_updates(),
            Self::ModTrigger(consumer) => consumer.has_updates(),
            Self::Glider(consumer) => consumer.has_updates(),
        }
    }
}
