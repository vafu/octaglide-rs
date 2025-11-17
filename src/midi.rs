use heapless::Deque;
use log::info;
use midi_msg::{MidiMsg, ReceiverContext};
use rtic_sync::channel::Sender;
use teensy4_bsp::{hal::lpuart::Status, ral};

use crate::{
    app::MidiUart,
    core::{Input as CoreIn, MidiEvent},
    midi_fmt::MidiFmt,
};

// TODO: consider changing midi BAUD (elektron Turbo), might need to update buf size.
const MIDI_BUF_SIZE: usize = 32;

pub struct MidiBus {
    uart: MidiUart,

    tx_ctx: Option<u8>,
    tx_buf: Deque<u8, MIDI_BUF_SIZE>,

    rx_ctx: ReceiverContext,
    rx_buf: Deque<u8, MIDI_BUF_SIZE>,

    core_sender: Sender<'static, CoreIn, 16>,
}

impl MidiBus {
    pub fn new(uart: MidiUart, core_sender: Sender<'static, CoreIn, 16>) -> Self {
        Self {
            uart,
            core_sender,
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
                        let input = CoreIn::Process(MidiEvent::from_user(Ok(msg)));
                        if let Err(e) = self.core_sender.try_send(input) {
                            log::error!("[MidiBus:CoreIn] {:?}", e);
                        }
                        self.drain_rx_queue(len);
                    }

                    Err(midi_msg::ParseError::UnexpectedEnd) => {
                        break;
                    }

                    Err(e) => {
                        let input = CoreIn::Process(MidiEvent::from_user(Err(e)));
                        if let Err(e) = self.core_sender.try_send(input) {
                            log::error!("[MidiBus:CoreIn] {:?}", e);
                        }
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

    pub fn send(&mut self, msg: &MidiMsg) {
        let formatted = alloc::format!("{}", MidiFmt(msg));
        if !formatted.is_empty() {
            info!(">>> {}", formatted);
        }

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
            if let Err(e) = self.tx_buf.push_back(byte) {
                log::error!("error pushing bytes {:?}", e)
            }
        }
        if !self.tx_buf.is_empty() {
            unsafe {
                ral::modify_reg!(ral::lpuart, &*ral::lpuart::LPUART6, CTRL, TIE: 1);
            }
        }
    }

    fn drain_rx_queue(&mut self, len: usize) {
        for _ in 0..len {
            unsafe {
                self.rx_buf.pop_front_unchecked();
            }
        }
    }
}
