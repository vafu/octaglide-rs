use futures::FutureExt;
use log::info;
use rtic_monotonics::{
    Monotonic,
    systick::{ExtU32, Systick, fugit::Instant},
};
use rtic_sync::{channel::Receiver, make_channel};

use crate::{
    anim::modulators::{Messages, Modulator},
    app::AnimatorSender,
};

use super::modulators::Modulation;

#[derive(Debug)]
pub enum Cmd {
    Start(Modulator),
    Duration(u32),
    // Looping(bool),
    Stop,
}

#[derive(Debug)]
pub struct Animator {
    rx: Receiver<'static, Cmd, 1>,
    looping: bool,
    duration: u32,
    state: State,
    // TODO: implement depth control (see roadmap) - will control animation intensity
    _depth: f32,
    modulator: Option<Modulator>,
}

#[derive(Debug)]
enum State {
    Idle,
    Animating {
        last_updated: Instant<u32, 1, 1000>,
        progress_at: f32,
    },
}

const MSG_INTERVAL_MS: u32 = 5;

impl Animator {
    pub fn new() -> (Self, AnimatorSender) {
        let (tx, rx) = make_channel!(Cmd, 1);
        (
            Animator {
                rx,
                looping: false,
                duration: 100,
                state: State::Idle,
                _depth: 1.0, // Default depth (full intensity)
                modulator: None,
            },
            tx,
        )
    }

    pub async fn tick(&mut self) -> Messages {
        if self.duration == 0 {
            return self.recv_cmd().await;
        }
        match self.state {
            State::Idle => self.recv_cmd().await,

            State::Animating {
                last_updated,
                progress_at: progress,
            } => futures::select_biased! {
                new_msg = self.recv_cmd().fuse() => new_msg,
                _ = Systick::delay(MSG_INTERVAL_MS.millis()).fuse() => {
                    let now = Systick::now();
                    let elapsed = now - last_updated;
                    let progress_delta = elapsed.to_millis() as f32 / self.duration as f32;
                    let new_progress = (progress + progress_delta).clamp(0.0, 1.0);
                    if new_progress >= 1.0 {
                        self.state = if self.looping {
                            State::Animating { last_updated: now, progress_at: 0.0 }
                        } else {
                            info!("anim end");
                            State::Idle
                        };
                    } else {
                        self.state = State::Animating {
                            last_updated: now,
                            progress_at: new_progress,
                        }
                    }
                    self.modulator.as_mut()?.animate(new_progress, 1.0, 1.0)
                }
            },
        }
    }

    async fn recv_cmd(&mut self) -> Messages {
        let Ok(cmd) = self.rx.recv().await else {
            log::error!("error receiving Animator cmd");
            return None;
        };
        match cmd {
            Cmd::Start(modulator) => {
                // Reset the old modulator before replacing it
                let mut messages = if let Some(old_mod) = self.modulator.as_mut() {
                    old_mod.reset().unwrap_or(heapless::Vec::new())
                } else {
                    heapless::Vec::new()
                };

                self.state = State::Animating {
                    last_updated: Systick::now(),
                    progress_at: 0.0,
                };
                self.modulator = Some(modulator);

                // Append initial animation messages to reset messages
                if let Some(new_msgs) = self.modulator.as_mut()?.animate(0.0, 0.0, 0.0) {
                    for msg in new_msgs {
                        let _ = messages.push(msg);
                    }
                }

                Some(messages)
            }
            Cmd::Stop => match self.state {
                State::Animating { .. } => {
                    self.state = State::Idle;
                    self.modulator.as_mut()?.reset()
                }
                State::Idle => self.modulator.as_mut()?.reset(),
            },
            Cmd::Duration(dur) => {
                self.duration = dur;
                if self.duration != 0 {
                    return None;
                }
                let State::Animating { .. } = self.state else {
                    return None;
                };

                self.modulator.as_mut()?.reset()
            } // EngineMessage::Looping(looping) => {
              //
              //     self.looping = looping;
              //     if looping && matches!(self.state, State::Idle) {
              //         self.state = State::Animating {
              //             last_updated: Systick::now(),
              //             progress: 0.0,
              //         };
              //         Some(self.function.reset())
              //     } else {
              //         None
              //     }
              // }
        }
    }
}


