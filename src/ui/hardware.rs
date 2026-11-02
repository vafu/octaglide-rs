use rtic_monotonics::{
    Monotonic,
    systick::{ExtU32, Systick},
};

use super::{ButtonDebouncer, ButtonEdge, ButtonReceiver, ButtonSender, ENCODER_BUTTON_INDEX};
use crate::{
    app::CoreSender,
    core::{Input as CoreIn, normalize_adc_10bit},
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

pub async fn run_buttons(mut receiver: ButtonReceiver, mut sender: CoreSender) -> ! {
    use futures::future::{Either, select};

    let mut buttons = [const { ButtonDebouncer::new() }; ENCODER_BUTTON_INDEX as usize + 1];
    loop {
        let edge = if buttons.iter().any(ButtonDebouncer::release_pending) {
            let receive = receiver.recv();
            let delay = Systick::delay(5.millis());
            futures::pin_mut!(receive, delay);
            // Drain queued edges before confirming any release.
            match select(receive, delay).await {
                Either::Left((edge, _)) => edge.ok(),
                Either::Right(_) => None,
            }
        } else {
            receiver.recv().await.ok()
        };

        // Settle releases that preceded this edge, even if the task was delayed.
        let at_ms = edge.as_ref().map_or_else(
            || Systick::now().duration_since_epoch().to_millis(),
            |edge| edge.at_ms,
        );
        while let Some(index) = super::buttons::take_next_release(&mut buttons, at_ms) {
            sender.send(button_input(index as u8, false)).await.ok();
        }
        if let Some(edge) = edge {
            if let Some(button) = buttons.get_mut(edge.index as usize)
                && button.update(edge.at_ms, edge.pressed)
            {
                sender.send(button_input(edge.index, true)).await.ok();
            }
        }
    }
}

fn button_input(index: u8, pressed: bool) -> CoreIn {
    if index == ENCODER_BUTTON_INDEX {
        CoreIn::EncoderClick { pressed }
    } else {
        CoreIn::Button { index, pressed }
    }
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
                .try_send(CoreIn::Encoder {
                    increment: delta > 0,
                })
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
                    .send(CoreIn::Slider {
                        index: i as u8,
                        value: normalize_adc_10bit(value),
                        alternate: false,
                    })
                    .await
                    .ok();
            }
        }
        Systick::delay(50.millis()).await;
    }
}
