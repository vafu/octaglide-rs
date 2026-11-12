use super::{Consumer, Consumers, Input, OutputEvents, Voice, VoiceOutput};

impl Voice {
    pub(super) async fn process_input(&mut self, input: Input) -> OutputEvents {
        let chain = self.voicing.iter_mut().chain(self.modulations.iter_mut());
        run_chain(chain, input).await
    }

    pub(super) async fn process_lane(&mut self, lane: usize, input: Input) -> OutputEvents {
        let Some(chain) = self.modulations.get_mut(lane..) else {
            return OutputEvents::new();
        };
        run_chain(chain.iter_mut(), input).await
    }
}

async fn run_chain<'a>(
    mut chain: impl Iterator<Item = &'a mut Consumers>,
    input: Input,
) -> OutputEvents {
    let mut current = OutputEvents::new();
    if let Some(first) = chain.next() {
        dispatch_consumer(first, input, &mut current).await;
        let mut next = OutputEvents::new();
        for consumer in chain {
            if current.is_empty() {
                break;
            }
            for msg in core::mem::take(&mut current) {
                if let Some(input) = output_to_input(msg) {
                    dispatch_consumer(consumer, input, &mut next).await;
                } else {
                    next.push(msg).ok();
                }
            }
            core::mem::swap(&mut current, &mut next);
        }
    } else {
        if let Some(output) = input_to_output(input) {
            current.push(output).ok();
        }
    }
    current
}

fn dispatch_consumer<'a, C: Consumer>(
    consumer: &'a mut C,
    input: Input,
    out: &'a mut OutputEvents,
) -> impl core::future::Future<Output = ()> + 'a {
    async move {
        let consumed = consumer.consume(&input, out).await;
        if !consumed && let Some(output) = input_to_output(input) {
            if out.push(output).is_err() {
                log::error!("Voice output capacity exceeded");
            }
        }
    }
}

fn input_to_output(input: Input) -> Option<VoiceOutput> {
    match input {
        Input::NoteOn { voice, note, velocity } => Some(VoiceOutput::NoteOn { voice, note, velocity }),
        Input::NoteOff { voice, note, velocity } => Some(VoiceOutput::NoteOff { voice, note, velocity }),
        _ => None,
    }
}

fn output_to_input(output: VoiceOutput) -> Option<Input> {
    match output {
        VoiceOutput::NoteOn { voice, note, velocity } => Some(Input::NoteOn { voice, note, velocity }),
        VoiceOutput::NoteOff { voice, note, velocity } => Some(Input::NoteOff { voice, note, velocity }),
        _ => None,
    }
}
