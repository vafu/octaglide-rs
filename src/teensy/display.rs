//! SH1106 OLED driver loop.

use rtic_monotonics::systick::{ExtU32, Systick};
use sh1106::{Builder, displayrotation::DisplayRotation, mode::GraphicsMode};
use teensy4_bsp::board;

use crate::ui::{UiReader, draw_snapshot};

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

    oled.clear();
    oled.flush().ok();

    loop {
        let snapshot = updates.wait().await;
        oled.clear();
        draw_snapshot(&mut oled, &snapshot).ok();
        oled.flush().ok();
    }
}
