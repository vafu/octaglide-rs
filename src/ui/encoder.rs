/// Decodes a mechanical quadrature encoder into one delta per detent.
#[derive(Debug)]
pub struct EncoderDecoder {
    previous: u8,
    accumulator: i8,
}

impl EncoderDecoder {
    const DETENT_STATE: u8 = 0b11;
    const TRANSITIONS_PER_DETENT: i8 = 4;

    /// Creates a decoder synchronized to the current A/B pin levels.
    pub fn new(a_high: bool, b_high: bool) -> Self {
        Self {
            previous: state(a_high, b_high),
            accumulator: 0,
        }
    }

    /// Updates the decoder from current A/B levels and returns a completed detent.
    pub fn update(&mut self, a_high: bool, b_high: bool) -> Option<i8> {
        let current = state(a_high, b_high);
        let step = transition_delta(self.previous, current);

        if step == 0 {
            if current != self.previous {
                self.accumulator = 0;
            }
            self.previous = current;
            return None;
        }

        self.previous = current;
        self.accumulator = self
            .accumulator
            .saturating_add(step)
            .clamp(-Self::TRANSITIONS_PER_DETENT, Self::TRANSITIONS_PER_DETENT);

        if current != Self::DETENT_STATE {
            return None;
        }

        if self.accumulator >= Self::TRANSITIONS_PER_DETENT {
            self.accumulator = 0;
            Some(1)
        } else if self.accumulator <= -Self::TRANSITIONS_PER_DETENT {
            self.accumulator = 0;
            Some(-1)
        } else {
            None
        }
    }
}

fn state(a_high: bool, b_high: bool) -> u8 {
    ((a_high as u8) << 1) | b_high as u8
}

fn transition_delta(previous: u8, current: u8) -> i8 {
    match (previous, current) {
        (0b11, 0b01) | (0b01, 0b00) | (0b00, 0b10) | (0b10, 0b11) => 1,
        (0b11, 0b10) | (0b10, 0b00) | (0b00, 0b01) | (0b01, 0b11) => -1,
        _ => 0,
    }
}
