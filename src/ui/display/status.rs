use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_5X8},
    pixelcolor::BinaryColor,
    prelude::*,
    text::Text,
};

/// A capital U in the header's top-left corner. The frame is cleared
/// before drawing, so disconnected status leaves this corner blank.
pub(super) fn draw_usb_status<D: DrawTarget<Color = BinaryColor>>(
    display: &mut D,
    connected: bool,
) -> Result<(), D::Error> {
    if connected {
        let style = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
        Text::new("U", Point::new(0, 7), style).draw(display)?;
    }
    Ok(())
}
