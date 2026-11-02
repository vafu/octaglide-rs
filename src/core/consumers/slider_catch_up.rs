use crate::{
    core::{Input, OutputEvents, control_to_u8},
    ui::UiProvider,
};

use super::{Consumer, SliderCatchUp, SliderTarget};

impl<C> SliderCatchUp<C> {
    pub fn new(inner: C) -> Self {
        Self {
            inner,
            previous: [None; 4],
            picked_up: [false; 4],
            alternate: false,
        }
    }

    /// Re-arm on selection or mapping changes using the latest physical positions.
    pub fn set_slider_context(
        &mut self,
        positions: [Option<u16>; 4],
        alternate: bool,
        rearm: bool,
    ) {
        if rearm || self.alternate != alternate {
            self.previous = positions.map(|value| value.map(|value| control_to_u8(value, 0, 127)));
            self.picked_up = [false; 4];
            self.alternate = alternate;
        }
    }
}

impl<C: Consumer + SliderTarget> Consumer for SliderCatchUp<C> {
    async fn consume(&mut self, input: &Input, out: &mut OutputEvents) -> bool {
        if let Input::Slider {
            index,
            value,
            alternate,
        } = *input
            && (index as usize) < self.previous.len()
        {
            // Also support direct use of the wrapper without Core's context hook.
            if self.alternate != alternate {
                self.alternate = alternate;
                self.picked_up = [false; 4];
            }
            let slot = index as usize;
            let value = control_to_u8(value, 0, 127);
            let previous = self.previous[slot].replace(value);
            if let Some(target) = self.inner.slider_value(index, alternate) {
                // Another writer may have moved the target after pickup.
                if self.picked_up[slot] && previous.is_some_and(|value| value != target) {
                    self.picked_up[slot] = false;
                }
                if !self.picked_up[slot] {
                    let reached = value == target
                        || previous.is_some_and(|previous| {
                            (previous.min(value)..=previous.max(value)).contains(&target)
                        });
                    if !reached {
                        return true;
                    }
                    self.picked_up[slot] = true;
                }
            }
        }
        self.inner.consume(input, out).await
    }
}

impl<C: UiProvider> UiProvider for SliderCatchUp<C> {
    type Model = C::Model;

    fn ui_snapshot(&self) -> Self::Model {
        self.inner.ui_snapshot()
    }

    fn has_updates(&mut self) -> bool {
        self.inner.has_updates()
    }
}
