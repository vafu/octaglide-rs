use futures::FutureExt;
use log::debug;
use rtic_sync::channel::Receiver;

use super::{Clock, Messages, Modulation, Modulator};

#[derive(Debug)]
pub struct AnimationEngine {
    rx: Receiver<'static, Cmd, 1>,
    looping: bool,
    state: State,
    _depth: f32,
    modulator: Option<Modulator>,
}

#[derive(Debug)]
enum State {
    Idle,
    Animating {
        last_updated: u32,
        progress_at: f32,
    },
}

const MSG_INTERVAL_MS: u32 = 5;

impl AnimationEngine {
    pub fn new(rx: Receiver<'static, Cmd, 1>) -> Self {
        AnimationEngine {
            rx,
            looping: false,
            state: State::Idle,
            _depth: 1.0,
            modulator: None,
        }
    }

    pub async fn tick<C: Clock>(&mut self) -> Messages {
        match self.state {
            State::Idle => self.recv_cmd::<C>().await,
            State::Animating {
                last_updated,
                progress_at: progress,
            } => {
                let Some(modulator) = self.modulator.as_mut() else {
                    return None;
                };
                let duration = modulator.duration_ms().await;

                futures::select_biased! {
                    new_msg = self.recv_cmd::<C>().fuse() => new_msg,
                    _ = C::delay_ms(MSG_INTERVAL_MS).fuse() => {
                        let now = C::now_ms();
                        let elapsed = now.wrapping_sub(last_updated);
                        let progress_delta = if duration == 0 { 1.0 } else { elapsed as f32 / duration as f32 };
                        let new_progress = (progress + progress_delta).clamp(0.0, 1.0);
                        let res = self.modulator.as_mut()?.animate(new_progress, 1.0, 1.0).await;
                        if new_progress >= 1.0 {
                            self.state = if self.modulator.as_mut()?.next_stage().await || self.looping {
                                State::Animating { last_updated: now, progress_at: 0.0 }
                            } else {
                                debug!("anim end");
                                State::Idle
                            };
                        } else {
                            self.state = State::Animating {
                                last_updated: now,
                                progress_at: new_progress,
                            }
                        }
                        res
                    }
                }
            }
        }
    }

    async fn recv_cmd<C: Clock>(&mut self) -> Messages {
        let Ok(cmd) = self.rx.recv().await else {
            log::error!("error receiving Animator cmd");
            return None;
        };
        match cmd {
            Cmd::Start(modulator) => {
                let mut messages = if let Some(old_mod) = self.modulator.as_mut() {
                    old_mod.reset().await.unwrap_or(heapless::Vec::new())
                } else {
                    heapless::Vec::new()
                };
                self.state = State::Animating {
                    last_updated: C::now_ms(),
                    progress_at: 0.0,
                };
                self.modulator = Some(modulator);
                if let Some(new_msgs) = self.modulator.as_mut()?.animate(0.0, 0.0, 0.0).await {
                    for msg in new_msgs {
                        let _ = messages.push(msg);
                    }
                }
                Some(messages)
            }
            Cmd::Update(settings) => {
                if let Some(modulator) = self.modulator.as_mut() {
                    modulator.update(&settings);
                }
                None
            }
            Cmd::Stop => {
                let messages = match self.state {
                    State::Animating { .. } => {
                        self.state = State::Idle;
                        self.modulator.as_mut()?.reset().await
                    }
                    State::Idle => self.modulator.as_mut()?.reset().await,
                };
                messages
            }
        }
    }
}

/// One glide engine and two envelope engines per voice.
pub const ANIMATORS_PER_VOICE: usize = 3;

pub type Animator = rtic_sync::channel::Sender<'static, Cmd, 1>;

#[derive(Debug)]
pub struct VoiceAnimators {
    pub glide: Animator,
    pub envelopes: [Animator; 2],
}

#[derive(Debug)]
pub enum Cmd {
    Start(Modulator),
    Update(Settings),
    Stop,
}

#[derive(Clone, Copy, Debug)]
pub enum Settings {
    Envelope(crate::state::EnvelopeState),
    Glide(crate::state::GlideState),
}
