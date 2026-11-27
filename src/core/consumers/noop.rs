//! The inert occupant of an unimplemented slot.

use crate::core::{Input, Modulation, OutputEvents};

use super::{Consumer, Routing};

/// Forwards every event untouched. Used for the Reserved section and for any
/// slot that has no implementation yet.
#[derive(Debug)]
pub struct Noop;

impl Consumer for Noop {
    fn consume(&mut self, _input: &Input, _out: &mut OutputEvents) -> Routing {
        Routing::Forward
    }
}

impl Modulation for Noop {}
