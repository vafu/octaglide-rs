//! Rolling scope window.

use ringbuf::{
    HeapCons,
    traits::Consumer,
};

use eframe::egui::{self, Color32, Stroke};

use crate::{
    acq::{DISPLAY_CAPACITY, Sample, WINDOW_SECS},
    cli::{Mode, Options, Rgb},
    trace::Trace,
};

/// Repaint interval.
const FRAME_MS: u64 = 33;

pub struct ScopeApp {
    options: Options,
    /// Ring B: uniform samples produced by the acquisition stage.
    samples: HeapCons<Sample>,
    /// What the plot draws: a fixed-rate ring of values.
    trace: Trace,
    port_name: String,
    error: Option<String>,
}

impl ScopeApp {
    pub fn new(options: Options, samples: HeapCons<Sample>, port_name: String) -> Self {
        Self {
            options,
            samples,
            trace: Trace::with_capacity(DISPLAY_CAPACITY),
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
        // Cap the frame rate. Without this egui repaints as fast as the GPU
        // allows, which pins a core even when nothing changes.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(FRAME_MS));

        // Drain ring B into the display trace. Bounded and allocation-free:
        // the acquisition stage guarantees a uniform sample rate, so the plot
        // never depends on how bursty MIDI was.
        while let Some(value) = self.samples.try_pop() {
            self.trace.push(value);
        }

        let title = describe(&self.options, &self.port_name);
        let window = WINDOW_SECS;

        egui::Panel::top("header")
            .exact_size(18.0)
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.small(title);
                    ui.small(format!("{window:.0}s"));
                    if let Some(error) = &self.error {
                        ui.small(egui::RichText::new(error).color(Color32::RED));
                    }
                });
            });

        egui::CentralPanel::default().show(ui, |ui| {
            draw_plot(ui, &mut self.trace, self.options.color);
        });
    }
}

/// Window title for the configured mode.
pub fn describe(options: &Options, port_name: &str) -> String {
    match options.mode {
        Mode::Cc { cc, channel } => format!("CC {cc}  ch {}  [{}]", channel + 1, port_name),
        Mode::Notes { channel } => format!("notes  ch {}  [{}]", channel + 1, port_name),
    }
}

fn draw_plot(ui: &mut egui::Ui, trace: &mut Trace, color: Rgb) {
    let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 8, 10));

    let stroke = Stroke::new(1.5, Color32::from_rgb(color.r, color.g, color.b));

    // Horizontal guides at 0, 64 and 127.
    let grid = Stroke::new(1.0, Color32::from_rgb(30, 30, 36));
    for value in [0.0f32, 63.5, 127.0] {
        let y = value_to_y(rect, value);
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            grid,
        );
    }

    // Position in the buffer is time: the newest sample sits at the right edge.
    let capacity = trace.capacity() as f32;
    let (first, second) = trace.slices();
    let total = first.len() + second.len();
    if total < 2 {
        return;
    }

    let mut points = Vec::with_capacity(total);
    for (index, value) in first.iter().chain(second).enumerate() {
        let x = (index as f32 + 1.0) / capacity;
        points.push(egui::pos2(
            rect.left() + x * rect.width(),
            value_to_y(rect, *value),
        ));
    }
    painter.add(egui::Shape::line(points, stroke));
}

fn value_to_y(rect: egui::Rect, value: f32) -> f32 {
    let normalized = (value / 127.0).clamp(0.0, 1.0);
    rect.bottom() - normalized * rect.height()
}
