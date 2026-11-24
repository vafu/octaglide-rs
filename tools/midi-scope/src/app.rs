//! Rolling scope window.

use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use eframe::egui::{self, Color32, Stroke};

use crate::{
    cli::{Mode, Options, Rgb},
    midi::CcTracker,
    trace::Trace,
};

/// Shared state written by the MIDI callback and read by the UI.
pub type Shared = Arc<Mutex<CcTracker>>;

pub struct ScopeApp {
    options: Options,
    tracker: Shared,
    start: Instant,
    port_name: String,
    error: Option<String>,
}

impl ScopeApp {
    pub fn new(options: Options, tracker: Shared, port_name: String) -> Self {
        Self {
            options,
            tracker,
            start: Instant::now(),
            port_name,
            error: None,
        }
    }

    pub fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }
}

impl eframe::App for ScopeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Keep repainting so the trace scrolls even when MIDI is idle.
        ui.ctx().request_repaint();

        let now = self.start.elapsed().as_secs_f64();
        let mut tracker = self.tracker.lock().expect("tracker lock");
        tracker.trace.hold(now);

        let (title, window) = describe(&self.options, &self.port_name);
        let window = window.unwrap_or(tracker.trace.window());

        egui::Panel::top("header")
            .exact_size(18.0)
            .show_inside(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.small(title);
                    ui.small(format!("{window:.0}s"));
                    if let Some(error) = &self.error {
                        ui.small(egui::RichText::new(error).color(Color32::RED));
                    }
                });
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            draw_plot(ui, &tracker.trace, self.options.color, window);
        });
    }
}

/// Title text and optional window override for the configured mode.
pub fn describe(options: &Options, port_name: &str) -> (String, Option<f64>) {
    match options.mode {
        Mode::Cc { cc, channel } => (
            format!("CC {cc}  ch {}  [{}]", channel + 1, port_name),
            None,
        ),
        Mode::Notes { channel } => (
            format!("notes  ch {}  [{}]", channel + 1, port_name),
            None,
        ),
    }
}

fn draw_plot(ui: &mut egui::Ui, trace: &Trace, color: Rgb, window: f64) {
    let (response, painter) =
        ui.allocate_painter(ui.available_size(), egui::Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 8, 10));

    let stroke = Stroke::new(1.5, Color32::from_rgb(color.r, color.g, color.b));

    // Horizontal guides at 0, 64 and 127.
    let grid = Stroke::new(1.0, Color32::from_rgb(30, 30, 36));
    for value in [0.0f32, 63.5, 127.0] {
        let y = value_to_y(rect, value);
        painter.line_segment(
            [
                egui::pos2(rect.left(), y),
                egui::pos2(rect.right(), y),
            ],
            grid,
        );
    }

    let samples = trace.samples();
    if samples.len() < 2 {
        return;
    }
    let now = samples.last().map(|(at, _)| *at).unwrap_or(0.0);
    let start = now - window;
    let points: Vec<egui::Pos2> = samples
        .iter()
        .map(|(at, value)| {
            let x = ((at - start) / window).clamp(0.0, 1.0) as f32;
            egui::pos2(
                rect.left() + x * rect.width(),
                value_to_y(rect, *value),
            )
        })
        .collect();
    painter.add(egui::Shape::line(points, stroke));
}

fn value_to_y(rect: egui::Rect, value: f32) -> f32 {
    let normalized = (value / 127.0).clamp(0.0, 1.0);
    rect.bottom() - normalized * rect.height()
}
