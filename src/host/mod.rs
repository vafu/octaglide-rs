//! Host simulator entry point.

mod clock;
mod display;
mod input;
mod midi;

use std::time::Instant;

use embedded_graphics_simulator::{SimulatorEvent, sdl2::Keycode};
use static_cell::StaticCell;
use tokio::task::LocalSet;

use crate::{
    app_event::{AppEvent, AppSender},
    core::Core,
    runtime::{RuntimeStorage, run_animations, run_buttons, run_core, run_midi_dispatch},
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
        Box::leak(Box::new(Core::new(handles.output_sender.clone(), handles.animators)));
    let midi: &'static mut HostMidiBus = Box::leak(Box::new(HostMidiBus::new()));
    let mut display = HostDisplay::new();
    let mut input = HostInput::new();
    let start = Instant::now();

    let mut app_sender = handles.app_sender.clone();
    let mut button_sender = handles.button_sender.clone();
    let ui_reader = handles.ui_reader;
    let receiver = handles.app_receiver;

    let core_task = run_core(
        core,
        receiver,
        handles.output_sender.clone(),
        handles.ui_writer.clone(),
    );
    let animation_task =
        run_animations::<TokioClock>(handles.engines, handles.output_sender.clone());
    let button_task = run_buttons::<TokioClock>(handles.button_receiver, handles.app_sender);
    let midi_task =
        run_midi_dispatch(midi, handles.output_receiver, crate::midi::MidiController::new());

    tokio::task::spawn_local(core_task);
    tokio::task::spawn_local(animation_task);
    tokio::task::spawn_local(button_task);
    tokio::task::spawn_local(midi_task);

    let mut ui_reader = ui_reader;
    loop {
        tokio::select! {
            snapshot = ui_reader.wait() => display.render(&snapshot),
            _ = tokio::time::sleep(std::time::Duration::from_millis(16)) => {}
        }
        for event in display.events() {
            match event {
                SimulatorEvent::Quit => std::process::exit(0),
                SimulatorEvent::KeyDown { keycode, .. } => {
                    if let Some(key) = map_key(keycode) {
                        dispatch(
                            input.handle(key, true, start.elapsed().as_millis() as u32),
                            &mut app_sender,
                            &mut button_sender,
                        );
                    }
                }
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
