use rtic_monotonics::systick::{ExtU32, Systick};

use crate::{
    app::CoreSender,
    core::{
        Input as CoreIn,
        event::{EventPayload, HardwareControl, normalize_adc_10bit},
    },
};

pub fn handle_encoder_interrupt(cx: crate::app::encoder_isr::Context<'_>) {
    if cx.local.enc_a.is_triggered() {
        cx.local.enc_a.clear_triggered();
        let delta: i8 = if cx.local.enc_b.is_set() { 1 } else { -1 };
        cx.local
            .enc_sender
            .try_send(CoreIn::Hardware {
                control: HardwareControl::Encoder,
                payload: EventPayload::Delta(delta as i16),
            })
            .ok();
    }
    if cx.local.enc_click.is_triggered() {
        cx.local.enc_click.clear_triggered();
        cx.local
            .enc_sender
            .try_send(CoreIn::Hardware {
                control: HardwareControl::EncoderClick,
                payload: EventPayload::Delta(1),
            })
            .ok();
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
                let control = match i {
                    0 => HardwareControl::Slider0,
                    1 => HardwareControl::Slider1,
                    2 => HardwareControl::Slider2,
                    _ => HardwareControl::Slider3,
                };
                sender
                    .send(CoreIn::Hardware {
                        control,
                        payload: EventPayload::Control(normalize_adc_10bit(value)),
                    })
                    .await
                    .ok();
            }
        }
        Systick::delay(50.millis()).await;
    }
}
