//! midi-scope: rolling MIDI monitor for a single CC or channel.

mod app;
mod cli;
mod midi;
mod trace;

use std::sync::{Arc, Mutex};

use app::ScopeApp;
use cli::Mode;
use midi::CcTracker;

fn main() -> std::process::ExitCode {
    let options = match cli::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            return std::process::ExitCode::from(2);
        }
    };

    let (cc, channel) = match options.mode {
        Mode::Cc { cc, channel } => (cc, channel),
        Mode::Notes { .. } => {
            eprintln!("midi-scope: --notes is not implemented yet");
            return std::process::ExitCode::from(2);
        }
    };

    let verbose = options.verbose;
    let tracker = Arc::new(Mutex::new(CcTracker::new(cc, channel, 10.0)));
    let sink = Arc::clone(&tracker);
    let start = std::time::Instant::now();

    let connection = midi::open(&options.port, move |_timestamp, bytes, _data| {
        let now = start.elapsed().as_secs_f64();
        let mut tracker = match sink.lock() {
            Ok(tracker) => tracker,
            Err(_) => return,
        };
        let messages = midi::decode_messages(bytes);
        if verbose {
            for message in &messages {
                println!("{}", midi::describe(message));
            }
        }
        for change in midi::decode_control_changes(bytes) {
            tracker.apply(change, now);
        }
    });

    let (connection, port_name, open_error) = match connection {
        // ALSA reports "client:port name"; keep the trailing port label.
        Ok((connection, name)) => (
            Some(connection),
            name.rsplit(':').next().unwrap_or(&name).trim().to_owned(),
            None,
        ),
        Err(error) => (None, options.port.clone(), Some(error)),
    };
    if let Some(error) = &open_error {
        eprintln!("midi-scope: {error}");
    }

    let title = format!(
        "midi-scope {}",
        match options.mode {
            Mode::Cc { cc, channel } => format!("CC {cc} ch {}", channel + 1),
            Mode::Notes { channel } => format!("notes ch {}", channel + 1),
        }
    );

    let mut scope = ScopeApp::new(options, tracker, port_name);
    if let Some(error) = open_error {
        scope = scope.with_error(error);
    }

    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([420.0, 140.0])
            .with_always_on_top()
            .with_title(title),
        ..Default::default()
    };

    let result = eframe::run_native(
        "midi-scope",
        native_options,
        Box::new(move |_cc| Ok(Box::new(scope))),
    );

    // Hold the connection open for the lifetime of the window.
    drop(connection);

    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("midi-scope: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
