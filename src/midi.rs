use heapless::Vec;
use midi_msg::{MidiMsg, ReceiverContext};

use crate::app::MidiUart;

const MIDI_BUF_SIZE: usize = 32;

pub struct MidiBus {
    uart: MidiUart,
    tx_ctx: Option<u8>,
    rx_ctx: ReceiverContext,
    buf: Vec<u8, MIDI_BUF_SIZE>,
}

impl MidiBus {
    pub fn new(uart: MidiUart) -> Self {
        Self {
            uart,
            tx_ctx: None,
            rx_ctx: ReceiverContext::new(),
            buf: Vec::new(),
        }
    }

    pub fn read<F>(&mut self, on_message: F)
    where
        F: Fn(&mut Self, MidiMsg),
    {
        while let Ok(Some(byte)) = self.uart.try_read() {
            self.buf.push(byte).ok();
        }

        let mut consumed = 0;

        loop {
            let slice = &self.buf[consumed..];
            if slice.is_empty() {
                break;
            }

            match MidiMsg::from_midi_with_context(slice, &mut self.rx_ctx) {
                Ok((msg, len)) => {
                    log::info!("Received: {:?}", msg);
                    on_message(self, msg);
                    consumed += len;
                }
                Err(midi_msg::ParseError::UnexpectedEnd) => {
                    break;
                }
                Err(e) => {
                    log::warn!("MIDI Parse Error: {:?}. Discarding a byte.", e);
                    consumed += 1;
                }
            }
        }
        if consumed > 0 {
            let len = self.buf.len();
            self.buf.copy_within(consumed.., 0);
            self.buf.truncate(len - consumed);
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
}
