//! SDL-backed 128x64 OLED simulator.

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
use embedded_graphics_simulator::{
    BinaryColorTheme, OutputSettingsBuilder, SimulatorDisplay, Window,
};

use crate::ui::{DisplaySnapshot, draw_snapshot};

/// Owns the simulated display and window.
pub struct HostDisplay {
    display: SimulatorDisplay<BinaryColor>,
    window: Window,
}

impl HostDisplay {
    pub fn new() -> Self {
        let output_settings = OutputSettingsBuilder::new()
            .scale(4)
            .theme(BinaryColorTheme::OledBlue)
            .build();
        Self {
            display: SimulatorDisplay::new(Size::new(128, 64)),
            window: Window::new("octaglide", &output_settings),
        }
    }

    pub fn render(&mut self, snapshot: &DisplaySnapshot) {
        self.display.clear(BinaryColor::Off).ok();
        draw_snapshot(&mut self.display, snapshot).ok();
        self.window.update(&self.display);
    }

    pub fn events(&self) -> impl Iterator<Item = embedded_graphics_simulator::SimulatorEvent> + '_ {
        self.window.events()
    }
}
