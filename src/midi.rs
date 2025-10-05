use heapless::Deque;
use midi_msg::{MidiMsg, ParseError, ReceiverContext};

use crate::app::MidiUart;

// TODO: consider changing midi BAUD (elektron Turbo), might need to update buf size.
const MIDI_BUF_SIZE: usize = 8;

pub struct MidiBus {
    uart: MidiUart,
    tx_ctx: Option<u8>,
    rx_ctx: ReceiverContext,
    buf: Deque<u8, MIDI_BUF_SIZE>,
}

impl MidiBus {
    pub fn new(uart: MidiUart) -> Self {
        Self {
            uart,
            tx_ctx: None,
            rx_ctx: ReceiverContext::new(),
            buf: Deque::new(),
        }
    }

    pub fn read<F>(&mut self, on_message: F)
    where
        F: Fn(Result<MidiMsg, ParseError>),
    {
        while let Ok(Some(byte)) = self.uart.try_read() {
            self.buf.push_back(byte).ok();
        }

        loop {
            self.buf.make_contiguous();
            let slice = self.buf.as_slices().0;
            if slice.is_empty() {
                break;
            }

            match MidiMsg::from_midi_with_context(slice, &mut self.rx_ctx) {
                Ok((msg, len)) => {
                    on_message(Ok(msg));
                    self.drain_queue(len);
                }

                Err(midi_msg::ParseError::UnexpectedEnd) => {
                    break;
                }

                Err(e) => {
                    on_message(Err(e));
                    self.drain_queue(1);
                }
            }
        }
    }

    pub fn write(&mut self, msg: &MidiMsg) {
        let bytes = msg.to_midi();
        let is_channel_msg = msg.is_channel_mode();
        let ctx = bytes[0];

        self.write_bytes(if is_channel_msg && self.tx_ctx == Some(ctx) {
            &bytes[1..]
        } else {
            &bytes
        });

        if is_channel_msg {
            self.tx_ctx = Some(ctx);
        }
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            while !self.uart.try_write(byte) {
                core::hint::spin_loop();
            }
        }
    }

    fn drain_queue(&mut self, len: usize) {
        for _ in 0..len {
            // TODO: might use unchecked
            self.buf.pop_front();
        }
    }
}
