use core::fmt::Debug;

use enum_dispatch::enum_dispatch;
use heapless::Vec;

use super::consumers::envelope::Envelope;
use super::consumers::glide::Glide;
use super::{Settings, VoiceOutput};

pub type Messages = Option<Vec<VoiceOutput, 3>>;

#[enum_dispatch]
pub trait Modulation {
    fn update(&mut self, settings: &Settings);
    async fn duration_ms(&self) -> u32;
    async fn animate(&mut self, progress: f32, depth: f32, offset: f32) -> Messages;
    async fn reset(&mut self) -> Messages;
    async fn next_stage(&mut self) -> bool;
}

#[enum_dispatch(Modulation)]
#[derive(Debug)]
pub enum Modulator {
    Glide,
    Envelope,
}
