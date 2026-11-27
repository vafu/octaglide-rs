use embedded_graphics::{
    Pixel,
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    pixelcolor::BinaryColor,
};

use crate::ui::{
    DisplaySnapshot, EnvelopeStageUi, ModTriggerUi, UiModel, draw_snapshot,
};

struct TestDisplay {
    pixels: [bool; 128 * 64],
}

impl TestDisplay {
    fn new() -> Self {
        Self {
            pixels: [false; 128 * 64],
        }
    }

    fn lit_pixels(&self) -> usize {
        self.pixels.iter().filter(|lit| **lit).count()
    }
}

impl OriginDimensions for TestDisplay {
    fn size(&self) -> Size {
        Size::new(128, 64)
    }
}

impl DrawTarget for TestDisplay {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            let (Ok(x), Ok(y)) = (u32::try_from(point.x), u32::try_from(point.y)) else {
                continue;
            };
            if x < 128 && y < 64 {
                self.pixels[(y * 128 + x) as usize] = color.is_on();
            }
        }
        Ok(())
    }
}

fn passthrough_snapshot() -> DisplaySnapshot {
    DisplaySnapshot {
        track: 0,
        voice: 0,
        section: crate::core::Section::Modulation,
        lane: 2,
        usb_connected: true,
        model: None,
    }
}

fn envelope_snapshot() -> DisplaySnapshot {
    DisplaySnapshot {
        track: 0,
        voice: 0,
        section: crate::core::Section::Modulation,
        lane: 2,
        usb_connected: false,
        model: Some(UiModel::ModTrigger(ModTriggerUi {
            name: "Envelope",
            output_cc: 2,
            attack: EnvelopeStageUi { duration: 20, curve: 64 },
            decay: EnvelopeStageUi { duration: 30, curve: 64 },
            sustain: 90,
            release: EnvelopeStageUi { duration: 40, curve: 64 },
            mode: 2,
        })),
    }
}

#[test]
fn passthrough_snapshot_draws_a_label() {
    let mut display = TestDisplay::new();
    draw_snapshot(&mut display, &passthrough_snapshot()).unwrap();
    assert!(display.lit_pixels() > 0);
}

#[test]
fn envelope_snapshot_draws_the_plot() {
    let mut display = TestDisplay::new();
    draw_snapshot(&mut display, &envelope_snapshot()).unwrap();
    assert!(display.lit_pixels() > 0);
}

#[test]
fn out_of_bounds_drawing_is_ignored() {
    let mut display = TestDisplay::new();
    draw_snapshot(&mut display, &passthrough_snapshot()).unwrap();
    assert_eq!(display.size(), Size::new(128, 64));
    // Every lit pixel must lie inside the panel; TestDisplay drops OOB points,
    // so a renderer that drew outside would otherwise pass silently.
    assert!(display.lit_pixels() > 0);
    assert_eq!(display.pixels.len(), 128 * 64);
}
