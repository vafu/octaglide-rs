//! Button edge debounce loop.

use futures::future::{Either, select};

use crate::{
    app_event::{AppEvent, AppSender},
    core::Clock,
    ui::{ButtonDebouncer, ButtonReceiver, ENCODER_BUTTON_INDEX, UiEvent},
};

/// Turns raw edge interrupts into debounced UI button events.
pub async fn run_buttons<C: Clock>(mut receiver: ButtonReceiver, mut sender: AppSender) -> ! {
    let mut buttons = [const { ButtonDebouncer::new() }; ENCODER_BUTTON_INDEX as usize + 1];
    loop {
        let edge = if buttons.iter().any(ButtonDebouncer::release_pending) {
            let receive = receiver.recv();
            let delay = C::delay_ms(5);
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
        let at_ms = edge.as_ref().map_or_else(C::now_ms, |edge| edge.at_ms);
        while let Some(index) = crate::ui::take_next_release(&mut buttons, at_ms) {
            sender
                .send(AppEvent::Ui(button_input(index as u8, false)))
                .await
                .ok();
        }
        if let Some(edge) = edge
            && let Some(button) = buttons.get_mut(edge.index as usize)
            && button.update(edge.at_ms, edge.pressed)
        {
            sender
                .send(AppEvent::Ui(button_input(edge.index, true)))
                .await
                .ok();
        }
    }
}

fn button_input(index: u8, pressed: bool) -> UiEvent {
    if index == ENCODER_BUTTON_INDEX {
        UiEvent::EncoderClick { pressed }
    } else {
        UiEvent::Button { index, pressed }
    }
}
