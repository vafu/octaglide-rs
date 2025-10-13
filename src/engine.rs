use futures::FutureExt;
use midi_msg::{Channel, ChannelVoiceMsg, MidiMsg};
use rtic_monotonics::{
    Monotonic,
    systick::{ExtU32, Systick, fugit::Instant},
};
use rtic_sync::{
    channel::{Receiver, Sender},
    make_channel,
};

#[derive(Debug)]
pub enum EngineMessage {
    Start,
    Duration(u32),
    Looping(bool),
    Stop,
}

#[derive(Debug)]
pub struct Engine {
    rx: Receiver<'static, EngineMessage, 1>,
    ch: Channel,
    cc: u32,
    looping: bool,
    duration: u32,
    state: State,
}

#[derive(Debug)]
enum State {
    Idle,
    Animating {
        last_updated: Instant<u32, 1, 1000>,
        progress: f32,
    },
}

const MSG_INTERVAL_MS: u32 = 1;

impl Engine {
    pub fn new(ch: Channel, cc: u32) -> (Self, Sender<'static, EngineMessage, 1>) {
        let (tx, rx) = make_channel!(EngineMessage, 1);
        (
            Engine {
                rx,
                cc,
                ch,
                looping: false,
                duration: 100,
                state: State::Idle,
            },
            tx,
        )
    }

    pub async fn tick(&mut self) -> Option<MidiMsg> {
        if self.duration == 0 {
            return self.recv_cmd().await;
        }
        match self.state {
            State::Idle => self.recv_cmd().await,

            State::Animating {
                last_updated,
                progress,
            } => futures::select_biased! {
                new_msg = self.recv_cmd().fuse() => new_msg,
                _ = Systick::delay(MSG_INTERVAL_MS.millis()).fuse() => {
                    let now = Systick::now();
                    let elapsed = now - last_updated;
                    let progress_delta = elapsed.to_millis() as f32 / self.duration as f32;
                    let new_progress = progress + progress_delta;
                    if new_progress >= 1.0 {
                        self.state = if self.looping {
                            State::Animating { last_updated: now, progress: 0.0 }
                        } else {
                            State::Idle
                        };
                    } else {
                        // Persist the new progress for the next tick
                        self.state = State::Animating {
                            last_updated: now,
                            progress: new_progress,
                        }
                    }
                    Some(self.calc_msg(new_progress))
                }
            },
        }
    }

    async fn recv_cmd(&mut self) -> Option<MidiMsg> {
        match self.rx.recv().await {
            Ok(EngineMessage::Start) => {
                self.state = State::Animating {
                    last_updated: Systick::now(),
                    progress: 0.0,
                };
                // self.looping = true;
                Some(self.calc_msg(0.0))
            }
            Ok(EngineMessage::Stop) => {
                self.state = State::Idle;
                Some(self.calc_msg(0.0))
            }
            Ok(EngineMessage::Duration(dur)) => {
                self.duration = dur;
                if self.duration == 0 {
                    Some(self.calc_msg(0.0))
                } else {
                    None
                }
            }
            Ok(EngineMessage::Looping(looping)) => {
                self.looping = looping;
                if looping && matches!(self.state, State::Idle) {
                    self.state = State::Animating {
                        last_updated: Systick::now(),
                        progress: 0.0,
                    };
                    Some(self.calc_msg(0.0))
                } else {
                    None
                }
            }
            Err(e) => {
                log::error!("Engine Error [ch:{}, cc:{}] -> {:?}", self.ch, self.cc, e);
                None
            }
        }
    }

    fn calc_msg(&self, progress: f32) -> MidiMsg {
        const PITCH_BEND_CENTER: f32 = 8192.0;
        let progress = progress.clamp(0.0, 1.0);
        let bend: u16 = (progress * PITCH_BEND_CENTER) as u16;
        MidiMsg::ChannelVoice {
            channel: self.ch,
            msg: ChannelVoiceMsg::PitchBend { bend },
        }
    }
}
