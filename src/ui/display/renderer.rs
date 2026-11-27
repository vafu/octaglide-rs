//! Platform-independent OLED frame drawing.

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_5X8},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Line, PrimitiveStyle},
    text::{Alignment, Baseline, Text, TextStyleBuilder},
};

use super::status::draw_usb_status;
use crate::ui::{DisplaySnapshot, UiModel, envelope_plot_level};

/// Draws one complete frame for the given snapshot.
pub fn draw_snapshot<D>(display: &mut D, snapshot: &DisplaySnapshot) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor>,
{
    let line_style = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
    let text_style = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
    let usb_connected = snapshot.usb_connected;

    let Some(current) = snapshot.model else {
        draw_usb_status(display, usb_connected)?;
        draw_heading(display, snapshot, text_style)?;
        Text::new("Empty", Point::new(0, 22), text_style).draw(display)?;
        return Ok(());
    };

    if let UiModel::Octave(model) = current {
        draw_usb_status(display, usb_connected)?;
        draw_heading(display, snapshot, text_style)?;
        use core::fmt::Write;
        let mut label = heapless::String::<32>::new();
        let _ = write!(label, "Octave: {} st", model.shift);
        Text::new(&label, Point::new(0, 22), text_style).draw(display)?;
        return Ok(());
    }

    if let UiModel::Glider(model) = current {
        draw_usb_status(display, usb_connected)?;
        draw_heading(display, snapshot, text_style)?;
        use core::fmt::Write;
        let mut label = heapless::String::<32>::new();
        let _ = write!(label, "Glide: {} ms", model.duration_ms);
        Text::new(&label, Point::new(0, 22), text_style).draw(display)?;
        return Ok(());
    }

    let UiModel::ModTrigger(current) = current else {
        return Ok(());
    };

    draw_usb_status(display, usb_connected)?;

    let mode_str = match current.mode {
        0 => "AD",
        1 => "AR",
        _ => "ADSR",
    };
    draw_heading(display, snapshot, text_style)?;
    Text::with_text_style(
        mode_str,
        Point::new(64, 63),
        text_style,
        TextStyleBuilder::new()
            .alignment(Alignment::Center)
            .baseline(Baseline::Bottom)
            .build(),
    )
    .draw(display)?;

    const TOP: i32 = 9;
    const BOT: i32 = 54;
    const H: i32 = BOT - TOP;

    let to_y = |lv: f32| BOT - (lv.clamp(0.0, 1.0) * H as f32) as i32;

    let mut prev_y = to_y(envelope_plot_level(&current, 0));
    for x in 1u32..128 {
        let y = to_y(envelope_plot_level(&current, x));
        Line::new(Point::new(x as i32 - 1, prev_y), Point::new(x as i32, y))
            .into_styled(line_style)
            .draw(display)?;
        prev_y = y;
    }

    Ok(())
}

fn draw_heading<D>(
    display: &mut D,
    snapshot: &DisplaySnapshot,
    text_style: MonoTextStyle<'_, BinaryColor>,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor>,
{
    use core::fmt::Write;
    let mut heading = heapless::String::<32>::new();
    let _ = write!(
        heading,
        "T{} V{} {}{}",
        snapshot.track + 1,
        snapshot.voice + 1,
        snapshot.section.label(),
        snapshot.lane + 1,
    );
    Text::with_alignment(&heading, Point::new(127, 7), text_style, Alignment::Right)
        .draw(display)?;
    Ok(())
}
