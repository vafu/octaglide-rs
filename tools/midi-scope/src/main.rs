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
    let _logger = match init_logging(verbose) {
        Ok(handle) => handle,
        Err(error) => {
            eprintln!("midi-scope: could not start logger: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let tracker = Arc::new(Mutex::new(CcTracker::new(cc, channel, 10.0)));
    let sink = Arc::clone(&tracker);
    let start = std::time::Instant::now();

    let connection = midi::open(&options.port, move |_timestamp, bytes, _data| {
        let now = start.elapsed().as_secs_f64();
        let changes = midi::decode_control_changes(bytes);

        // Log and parse outside the lock: the UI thread needs it every frame,
        // and a terminal write must never stall rendering.
        if verbose {
            for message in midi::decode_messages(bytes) {
                log::info!("{}", midi::describe(&message));
            }
        }

        if let Ok(mut tracker) = sink.lock() {
            for change in changes {
                tracker.apply(change, now);
            }
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

/// Prints just the message, no level or timestamp noise.
fn bare_format(
    out: &mut dyn std::io::Write,
    _now: &mut flexi_logger::DeferredNow,
    record: &log::Record<'_>,
) -> std::io::Result<()> {
    writeln!(out, "{}", record.args())
}

/// Starts a buffered stdout logger.
///
/// `BufferAndFlush` batches lines and flushes on a timer, so a burst of MIDI
/// messages costs one write per interval instead of one syscall per line.
fn init_logging(verbose: bool) -> Result<flexi_logger::LoggerHandle, flexi_logger::FlexiLoggerError> {
    use flexi_logger::{Logger, WriteMode};

    // `try_with_str` takes a spec: "info" when monitoring, "off" for --quiet.
    let spec = if verbose { "info" } else { "off" };
    Logger::try_with_str(spec)?
        .log_to_stdout()
        .format(bare_format)
        .write_mode(WriteMode::BufferAndFlushWith(
            64 * 1024,
            std::time::Duration::from_millis(100),
        ))
        .start()
}
