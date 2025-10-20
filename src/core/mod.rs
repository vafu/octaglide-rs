mod consumers;
mod transformers;

use alloc::{boxed::Box, vec::Vec};
use log::info;
use midi_msg::{MidiMsg, ParseError};

use crate::{
    anim::engine::Cmd,
    app::Dispatcher,
    core::{
        consumers::{Consumer, Glider},
        transformers::{MidiTransformer, OctaveShifter},
    },
};

pub struct Core {
    dispatcher: Dispatcher,
    transformers: Vec<Box<dyn MidiTransformer>>,
    consumers: Vec<Box<dyn Consumer>>,
}

#[derive(Debug)]
pub enum Input {
    ProcessMidi(Result<MidiMsg, ParseError>),
}

#[derive(Debug)]
pub enum Output {
    SendMidi(MidiMsg),
    Animate(Cmd),
    BlinkLed,
}

impl Core {
    pub fn new(dispatcher: Dispatcher) -> Self {
        let transformers: Vec<Box<dyn MidiTransformer>> = vec![Box::new(OctaveShifter::new())];
        let consumers: Vec<Box<dyn Consumer>> = vec![Box::new(Glider::new())];
        Core {
            dispatcher,
            transformers,
            consumers,
        }
    }

    pub async fn process(&mut self, input: Input) {
        match input {
            Input::ProcessMidi(midi_msg) => self.process_midi(midi_msg).await,
        }
    }

    async fn process_midi(&mut self, msg: Result<MidiMsg, midi_msg::ParseError>) {
        match msg {
            Ok(msg) => {
                let msg = self
                    .transformers
                    .iter_mut()
                    .try_fold(msg, |m, p| p.process(m));
                info!("<<< {:?}", &msg);

                if let Some(msg) = msg {
                    for c in self.consumers.iter_mut() {
                        for out in c.consume(&msg) {
                            self.dispatcher.dispatch(out).await
                        }
                    }
                }
            }
            Err(e) => info!("Midi: {:?}", e),
        }
    }
}
