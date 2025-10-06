use heapless::Deque;
use midi_msg::{MidiMsg, ParseError, ReceiverContext};
use teensy4_bsp::{hal::lpuart::Status, ral};

use crate::app::MidiUart;

// TODO: consider changing midi BAUD (elektron Turbo), might need to update buf size.
const MIDI_BUF_SIZE: usize = 8;

pub trait MidiHandler {
    fn on_message(&self, msg: Result<MidiMsg, ParseError>);
}

pub struct MidiBus<H: MidiHandler> {
    uart: MidiUart,

    tx_ctx: Option<u8>,
    tx_buf: Deque<u8, MIDI_BUF_SIZE>,

    rx_ctx: ReceiverContext,
    rx_buf: Deque<u8, MIDI_BUF_SIZE>,

    on_message: H,
}

impl<H: MidiHandler> MidiBus<H> {
    pub fn new(uart: MidiUart, on_message: H) -> Self {
        Self {
            uart,
            on_message,
            tx_ctx: None,
            tx_buf: Deque::new(),
            rx_ctx: ReceiverContext::new(),
            rx_buf: Deque::new(),
        }
    }

    pub fn handle_interrupt(&mut self) {
        let status = self.uart.status();
        if status.contains(Status::RECEIVE_FULL) {
            while let Ok(Some(byte)) = self.uart.try_read() {
                self.rx_buf.push_back(byte).ok();
            }

            loop {
                if self.rx_buf.is_empty() {
                    break;
                }
                self.rx_buf.make_contiguous();
                let slice = self.rx_buf.as_slices().0;

                match MidiMsg::from_midi_with_context(slice, &mut self.rx_ctx) {
                    Ok((msg, len)) => {
                        self.on_message.on_message(Ok(msg));
                        self.drain_rx_queue(len);
                    }

                    Err(midi_msg::ParseError::UnexpectedEnd) => {
                        break;
                    }

                    Err(e) => {
                        self.on_message.on_message(Err(e));
                        self.drain_rx_queue(1);
                    }
                }
            }
        }

        if status.contains(Status::TRANSMIT_EMPTY) {
            if let Some(byte) = self.tx_buf.pop_front() {
                self.uart.write_byte(byte);
            } else {
                unsafe {
                    ral::modify_reg!(ral::lpuart, &*ral::lpuart::LPUART6, CTRL, TIE: 0);
                }
            }
        }
    }

    pub fn send(&mut self, msg: MidiMsg) {
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
            self.tx_buf.push_back(byte).unwrap();
        }
        if !self.tx_buf.is_empty() {
            unsafe {
                ral::modify_reg!(ral::lpuart, &*ral::lpuart::LPUART6, CTRL, TIE: 1);
            }
        }
    }

    fn drain_rx_queue(&mut self, len: usize) {
        for _ in 0..len {
            // TODO: might use unchecked
            self.rx_buf.pop_front();
        }
    }
}
