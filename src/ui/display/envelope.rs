use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_5X8},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Line, PrimitiveStyle},
    text::{Alignment, Baseline, Text, TextStyleBuilder},
};
use rtic_monotonics::systick::{ExtU32, Systick};
use sh1106::{Builder, displayrotation::DisplayRotation, mode::GraphicsMode};
use teensy4_bsp::board;

use super::status::draw_usb_status;
use crate::ui::{UiModel, UiReader, envelope_plot_level};

pub async fn run_envelope_display(i2c: board::Lpi2c1, mut updates: UiReader) {
    Systick::delay(1000.millis()).await;

    let mut oled: GraphicsMode<_> = Builder::new()
        .with_rotation(DisplayRotation::Rotate180)
        .connect_i2c(i2c)
        .into();
    if oled.init().is_err() {
        log::error!("OLED init failed");
        return;
    }
    log::info!("OLED ok");

    let line_style = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
    let text_style = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);

    oled.clear();
    oled.flush().ok();

    loop {
        let snapshot = updates.wait().await;
        let usb_connected = snapshot.usb_connected;
        let current = match snapshot.model {
            Some(UiModel::ModTrigger(model)) => model,
            model => {
                use core::fmt::Write;
                let mut label = heapless::String::<64>::new();
                match model {
                    Some(UiModel::Octave(model)) => {
                        let _ = write!(label, "Octave: {} semitones", model.shift);
                    }
                    Some(UiModel::Glider(model)) => {
                        let _ = write!(label, "Glide: {} ms", model.duration_ms);
                    }
                    _ => {
                        let _ = write!(label, "Passthrough");
                    }
                }
                oled.clear();
                draw_usb_status(&mut oled, usb_connected).ok();
                let mut heading = heapless::String::<32>::new();
                let _ = write!(
                    heading,
                    "T{} V{} L{}",
                    snapshot.track + 1,
                    snapshot.voice + 1,
                    snapshot.lane + 1
                );
                Text::with_alignment(&heading, Point::new(127, 7), text_style, Alignment::Right)
                    .draw(&mut oled)
                    .ok();
                Text::new(&label, Point::new(0, 22), text_style)
                    .draw(&mut oled)
                    .ok();
                oled.flush().ok();
                continue;
            }
        };

        let mode = current.mode;

        oled.clear();
        draw_usb_status(&mut oled, usb_connected).ok();

        let mode_str = match mode {
            0 => "AD",
            1 => "AR",
            _ => "ADSR",
        };
        use core::fmt::Write;
        let mut heading = heapless::String::<32>::new();
        let _ = write!(
            heading,
            "T{} V{} L{}",
            snapshot.track + 1,
            snapshot.voice + 1,
            snapshot.lane + 1,
        );
        Text::with_alignment(&heading, Point::new(127, 7), text_style, Alignment::Right)
            .draw(&mut oled)
            .ok();
        Text::with_text_style(
            mode_str,
            Point::new(64, 63),
            text_style,
            TextStyleBuilder::new()
                .alignment(Alignment::Center)
                .baseline(Baseline::Bottom)
                .build(),
        )
        .draw(&mut oled)
        .ok();

        const TOP: i32 = 9;
        const BOT: i32 = 54; // Reserve the bottom text row for the envelope mode.
        const H: i32 = BOT - TOP;

        let to_y = |lv: f32| BOT - (lv.clamp(0.0, 1.0) * H as f32) as i32;

        let mut prev_y = to_y(envelope_plot_level(&current, 0));
        for x in 1u32..128 {
            let y = to_y(envelope_plot_level(&current, x));
            Line::new(Point::new(x as i32 - 1, prev_y), Point::new(x as i32, y))
                .into_styled(line_style)
                .draw(&mut oled)
                .ok();
            prev_y = y;
        }

        oled.flush().ok();
    }
}
