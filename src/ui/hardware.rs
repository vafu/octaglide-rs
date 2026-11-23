use rtic_monotonics::{
    Monotonic,
    systick::{ExtU32, Systick},
};

use super::{ButtonDebouncer, ButtonEdge, ButtonReceiver, ButtonSender, ENCODER_BUTTON_INDEX};
use crate::{
    app::CoreSender,
    app_event::AppEvent,
    core::normalize_adc_10bit,
    ui::UiEvent,
};
use teensy4_bsp::hal::gpio;

pub fn handle_buttons_gpio3_interrupt(cx: crate::app::buttons_gpio3_isr::Context<'_>) {
    let now_ms = Systick::now().duration_since_epoch().to_millis();
    handle_button(cx.local.button1, cx.local.buttons_gpio3_sender, 0, now_ms);
    handle_button(cx.local.button3, cx.local.buttons_gpio3_sender, 2, now_ms);
    handle_button(cx.local.button4, cx.local.buttons_gpio3_sender, 3, now_ms);
}

pub fn handle_buttons_gpio4_interrupt(cx: crate::app::buttons_gpio4_isr::Context<'_>) {
    let now_ms = Systick::now().duration_since_epoch().to_millis();
    handle_button(cx.local.button2, cx.local.buttons_gpio4_sender, 1, now_ms);
}

fn handle_button<P>(pin: &gpio::Input<P>, sender: &mut ButtonSender, index: u8, now_ms: u32) {
    if !pin.is_triggered() {
        return;
    }
    pin.clear_triggered();
    // Interrupts only capture edges; the button task owns debounce and state.
    sender
        .try_send(ButtonEdge {
            index,
            pressed: !pin.is_set(),
            at_ms: now_ms,
        })
        .ok();
}

pub fn handle_encoder_interrupt(cx: crate::app::encoder_isr::Context<'_>) {
    let encoder_triggered = cx.local.enc_a.is_triggered() || cx.local.enc_b.is_triggered();
    if cx.local.enc_a.is_triggered() {
        cx.local.enc_a.clear_triggered();
    }
    if cx.local.enc_b.is_triggered() {
        cx.local.enc_b.clear_triggered();
    }

    if encoder_triggered {
        let delta = cx
            .local
            .encoder
            .update(cx.local.enc_a.is_set(), cx.local.enc_b.is_set());
        if let Some(delta) = delta {
            cx.local
                .enc_sender
                .try_send(AppEvent::Ui(UiEvent::Encoder {
                    increment: delta > 0,
                }))
                .ok();
        }
    }

    handle_button(
        cx.local.enc_click,
        cx.local.enc_button_sender,
        ENCODER_BUTTON_INDEX,
        Systick::now().duration_since_epoch().to_millis(),
    );
}

pub async fn read_sliders(cx: crate::app::read_sliders::Context<'_>, mut sender: CoreSender) -> ! {
    const DEADBAND: u16 = 4;

    loop {
        let readings = [
            cx.local.adc1.read_blocking(cx.local.slider0),
            cx.local.adc1.read_blocking(cx.local.slider1),
            cx.local.adc2.read_blocking(cx.local.slider2),
            cx.local.adc2.read_blocking(cx.local.slider3),
        ];
        for (i, &value) in readings.iter().enumerate() {
            let prev = &mut cx.local.prev[i];
            if value.abs_diff(*prev) > DEADBAND {
                *prev = value;
                // readings has exactly four entries, so i is always 0..=3.
                sender
                    .send(AppEvent::Ui(UiEvent::Slider {
                        index: i as u8,
                        value: normalize_adc_10bit(value),
                    }))
                    .await
                    .ok();
            }
        }
        Systick::delay(50.millis()).await;
    }
}
