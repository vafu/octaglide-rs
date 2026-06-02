use rtic_monotonics::systick::{ExtU32, Systick};

use crate::{app::CoreSender, core::Input as CoreIn};

pub fn handle_encoder_interrupt(cx: crate::app::encoder_isr::Context<'_>) {
    if cx.local.enc_a.is_triggered() {
        cx.local.enc_a.clear_triggered();
        let delta: i8 = if cx.local.enc_b.is_set() { 1 } else { -1 };
        cx.local
            .enc_sender
            .try_send(CoreIn::EncoderStep(delta))
            .ok();
    }
    if cx.local.enc_click.is_triggered() {
        cx.local.enc_click.clear_triggered();
        cx.local.enc_sender.try_send(CoreIn::EncoderClick).ok();
    }
}

pub async fn read_sliders(cx: crate::app::read_sliders::Context<'_>, mut sender: CoreSender) -> ! {
    const DEADBAND: u16 = 4;

    loop {
        let readings = [
            cx.local.adc1.read_blocking(cx.local.slider0),
            cx.local.adc1.read_blocking(cx.local.slider1),
            cx.local.adc1.read_blocking(cx.local.slider2),
            cx.local.adc1.read_blocking(cx.local.slider3),
        ];
        for (i, &value) in readings.iter().enumerate() {
            let prev = &mut cx.local.prev[i];
            if value.abs_diff(*prev) > DEADBAND {
                *prev = value;
                sender
                    .send(CoreIn::AnalogUpdate {
                        index: i as u8,
                        value,
                    })
                    .await
                    .ok();
            }
        }
        Systick::delay(50.millis()).await;
    }
}
