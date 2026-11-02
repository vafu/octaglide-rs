use super::ButtonDebouncer;

impl ButtonDebouncer {
    const RELEASE_DEBOUNCE_MS: u32 = 30;

    pub const fn new() -> Self {
        Self {
            latched: false,
            released_at_ms: None,
        }
    }

    /// Observe both GPIO edges. Release is confirmed separately after 30 ms.
    /// Repeated interrupts while held never emit another press.
    pub fn update(&mut self, now_ms: u32, pressed: bool) -> bool {
        if !pressed {
            self.released_at_ms.get_or_insert(now_ms);
            return false;
        }

        self.released_at_ms = None;
        let emit = !self.latched;
        self.latched = true;
        emit
    }

    /// True while a latched press has a release awaiting confirmation.
    pub fn release_pending(&self) -> bool {
        self.latched && self.released_at_ms.is_some()
    }

    /// Confirm a stable release once, and re-arm the next press.
    pub fn take_release(&mut self, now_ms: u32) -> bool {
        if self.release_pending()
            && self
                .released_at_ms
                .is_some_and(|since| now_ms.wrapping_sub(since) >= Self::RELEASE_DEBOUNCE_MS)
        {
            self.latched = false;
            self.released_at_ms = None;
            true
        } else {
            false
        }
    }
}

/// Confirm the oldest settled release first, preserving modifier/button order.
pub(super) fn take_next_release(buttons: &mut [ButtonDebouncer], now_ms: u32) -> Option<usize> {
    let index = buttons
        .iter()
        .enumerate()
        .filter_map(|(index, button)| {
            if !button.release_pending() {
                return None;
            }
            let age = now_ms.wrapping_sub(button.released_at_ms?);
            (age >= ButtonDebouncer::RELEASE_DEBOUNCE_MS).then_some((index, age))
        })
        .max_by_key(|&(_, age)| age)
        .map(|(index, _)| index)?;
    buttons[index].take_release(now_ms);
    Some(index)
}
