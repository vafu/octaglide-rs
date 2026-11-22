//! An inert modulation lane.

use crate::core::{Input, OutputEvents};

use super::Consumer;

/// Leaves every event for the next consumer or the voice output.
#[derive(Debug)]
pub struct Passthrough;

impl Consumer for Passthrough {
    async fn consume(&mut self, _input: &Input, _out: &mut OutputEvents) -> bool {
        false
    }
}
