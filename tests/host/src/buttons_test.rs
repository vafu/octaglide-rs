use futures::executor::block_on;
use futures::future::{select, Either};

use crate::{
    app_event::AppEvent,
    core::Clock,
    runtime::{RuntimeStorage, run_buttons},
    ui::{ButtonDebouncer, ButtonEdge, UiEvent},
};

struct TestClock;

impl Clock for TestClock {
    fn now_ms() -> u32 {
        0
    }

    async fn delay_ms(_ms: u32) {
        core::future::pending::<()>().await;
    }
}

#[test]
fn held_button_emits_one_press_and_one_release() {
    let mut button = ButtonDebouncer::new();
    assert!(button.update(0, true));
    assert!(!button.update(10, true));
    assert!(!button.update(20, false));
    assert!(!button.take_release(40));
    assert!(button.take_release(50));
    assert!(!button.take_release(60));
}

#[test]
fn run_buttons_forwards_a_press_as_a_ui_event() {
    block_on(async {
        let storage: &'static mut RuntimeStorage = Box::leak(Box::new(RuntimeStorage::new()));
        let mut handles = storage.split();
        handles
            .button_sender
            .try_send(ButtonEdge {
                index: 0,
                pressed: true,
                at_ms: 0,
            })
            .unwrap();

        let task = run_buttons::<TestClock>(handles.button_receiver, handles.app_sender);
        let receive = handles.app_receiver.recv();
        futures::pin_mut!(task);
        futures::pin_mut!(receive);

        match select(task, receive).await {
            Either::Right((
                Ok(AppEvent::Ui(UiEvent::Button {
                    index: 0,
                    pressed: true,
                })),
                _,
            )) => {}
            Either::Right((other, _)) => panic!("unexpected event: {:?}", other),
            Either::Left(_) => panic!("run_buttons finished unexpectedly"),
        }
    });
}
