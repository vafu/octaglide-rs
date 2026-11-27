//! Host simulator entry point.

mod clock;
mod display;
mod input;
mod midi;

use std::time::Instant;

use embedded_graphics_simulator::{
    SimulatorEvent,
    sdl2::{Keycode, MouseButton},
};
use static_cell::StaticCell;
use tokio::task::LocalSet;

use crate::{
    app_event::{AppEvent, AppSender},
    core::Core,
    runtime::{RuntimeStorage, run_buttons, run_core, run_midi_dispatch},
    ui::{ButtonSender, UiEvent},
};

use self::{
    clock::TokioClock,
    display::HostDisplay,
    input::{HostEvent, HostInput, HostKey},
    midi::HostMidiBus,
};

static STORAGE: StaticCell<RuntimeStorage> = StaticCell::new();

/// Runs the host application.
pub fn run() -> ! {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("build tokio runtime");

    runtime.block_on(async {
        let local = LocalSet::new();
        local.run_until(application()).await;
        std::process::exit(0);
    })
}

async fn application() {
    let handles = STORAGE.init(RuntimeStorage::new()).split();
    let core: &'static mut Core =
        Box::leak(Box::new(Core::new(handles.output_sender.clone())));
    let midi: &'static mut HostMidiBus = Box::leak(Box::new(HostMidiBus::new("octaglide", "octaglide-in", handles.app_sender.clone())));
    let mut display = HostDisplay::new();
    let mut input = HostInput::new();
    let start = Instant::now();

    let mut app_sender = handles.app_sender.clone();
    let mut button_sender = handles.button_sender.clone();
    let ui_reader = handles.ui_reader;
    let receiver = handles.app_receiver;

    let core_task = run_core::<TokioClock>(
        core,
        receiver,
        handles.output_sender.clone(),
        handles.ui_writer.clone(),
    );
    let button_task = run_buttons::<TokioClock>(handles.button_receiver, handles.app_sender);
    let midi_task =
        run_midi_dispatch(midi, handles.output_receiver, crate::midi::MidiController::new());

    tokio::task::spawn_local(core_task);
    tokio::task::spawn_local(button_task);
    tokio::task::spawn_local(midi_task);

    log::info!("host simulator ready: 128x64 OLED, keys A/S/D/F, Q/E, space, Z-M/comma, 1-5, U");

    // Push-based redraw: the UI signal fires only when `Controller::publish`
    // sees a changed snapshot, so a redraw costs one blit per actual change
    // rather than one per timer tick. The timer exists solely to pump SDL
    // events when no UI update arrives.
    // The window is not mapped when the first frame is pushed, and the
    // compositor clears it on map. Keep the newest snapshot and re-present it
    // once the surface exists; after that, redraw only on real changes.
    let mut ui_reader = ui_reader;
    let mut last_snapshot = None;
    let mut remap_deadline = Some(std::time::Instant::now() + std::time::Duration::from_millis(250));
    loop {
        tokio::select! {
            snapshot = ui_reader.wait() => {
                display.render(&snapshot);
                last_snapshot = Some(snapshot);
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(16)) => {
                if remap_deadline.is_some_and(|deadline| std::time::Instant::now() >= deadline) {
                    remap_deadline = None;
                    if let Some(snapshot) = &last_snapshot {
                        display.render(snapshot);
                    }
                }
            }
        }
        for event in display.events() {
            match event {
                SimulatorEvent::Quit => std::process::exit(0),
                SimulatorEvent::KeyDown {
                    keycode,
                    repeat: false,
                    ..
                } => {
                    if let Some(key) = map_key(keycode) {
                        dispatch(
                            input.handle(key, true, start.elapsed().as_millis() as u32),
                            &mut app_sender,
                            &mut button_sender,
                        );
                    }
                }
                SimulatorEvent::MouseButtonDown {
                    point,
                    mouse_btn: MouseButton::Left,
                } => {
                    dispatch(input.pointer_down(point), &mut app_sender, &mut button_sender);
                }
                SimulatorEvent::MouseMove { point } => {
                    dispatch(input.pointer_move(point), &mut app_sender, &mut button_sender);
                }
                SimulatorEvent::MouseButtonUp {
                    mouse_btn: MouseButton::Left,
                    ..
                } => input.pointer_up(),
                SimulatorEvent::KeyUp { keycode, .. } => {
                    if let Some(key) = map_key(keycode) {
                        dispatch(
                            input.handle(key, false, start.elapsed().as_millis() as u32),
                            &mut app_sender,
                            &mut button_sender,
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

fn dispatch(event: Option<HostEvent>, app: &mut AppSender, buttons: &mut ButtonSender) {
    match event {
        Some(HostEvent::App(event)) => {
            app.try_send(event).ok();
        }
        Some(HostEvent::Button(edge)) => {
            buttons.try_send(edge).ok();
        }
        Some(HostEvent::Encoder(event)) => {
            app.try_send(AppEvent::Ui(event)).ok();
        }
        None => {}
    }
}

fn map_key(key: Keycode) -> Option<HostKey> {
    Some(match key {
        Keycode::A => HostKey::Button(0),
        Keycode::S => HostKey::Button(1),
        Keycode::D => HostKey::Button(2),
        Keycode::F => HostKey::Button(3),
        Keycode::Q => HostKey::EncoderCcw,
        Keycode::E => HostKey::EncoderCw,
        Keycode::Space => HostKey::EncoderClick,
        Keycode::Z => HostKey::SliderDown(0),
        Keycode::X => HostKey::SliderUp(0),
        Keycode::C => HostKey::SliderDown(1),
        Keycode::V => HostKey::SliderUp(1),
        Keycode::B => HostKey::SliderDown(2),
        Keycode::N => HostKey::SliderUp(2),
        Keycode::M => HostKey::SliderDown(3),
        Keycode::Comma => HostKey::SliderUp(3),
        Keycode::Num1 => HostKey::Note(60),
        Keycode::Num2 => HostKey::Note(62),
        Keycode::Num3 => HostKey::Note(64),
        Keycode::Num4 => HostKey::Note(65),
        Keycode::Num5 => HostKey::Note(67),
        Keycode::U => HostKey::UsbToggle,
        _ => return None,
    })
}
