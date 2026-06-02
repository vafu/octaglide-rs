use core::sync::atomic::Ordering::Relaxed;

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_5X8},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Line, PrimitiveStyle},
    text::Text,
};
use rtic_monotonics::systick::{ExtU32, Systick};
use sh1106::{Builder, mode::GraphicsMode};
use teensy4_bsp::board;

use crate::anim::modulators::envelope::CONFIGS;

pub async fn run_envelope_display(i2c: board::Lpi2c3) {
    Systick::delay(1000.millis()).await;

    let mut oled: GraphicsMode<_> = Builder::new().connect_i2c(i2c).into();
    if oled.init().is_err() {
        log::error!("OLED init failed");
        return;
    }
    log::info!("OLED ok");

    let line_style = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
    let text_style = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);

    let mut last: (u32, u32, u8, u32, u8, u8, u8, u8) =
        (u32::MAX, u32::MAX, 0, u32::MAX, 0, 0, 0, u8::MAX);

    loop {
        let cfg = &CONFIGS[0];
        let a_dur = cfg.attack.duration.load(Relaxed) as u32;
        let d_dur = cfg.decay.duration.load(Relaxed) as u32;
        let s_val = cfg.sustain.load(Relaxed) as f32 / 127.0;
        let s_raw = cfg.sustain.load(Relaxed);
        let r_dur = cfg.release.duration.load(Relaxed) as u32;
        let a_crv = cfg.attack.curve.load(Relaxed);
        let d_crv = cfg.decay.curve.load(Relaxed);
        let r_crv = cfg.release.curve.load(Relaxed);
        let mode = cfg.mode.load(Relaxed);

        let current = (a_dur, d_dur, s_raw, r_dur, a_crv, d_crv, r_crv, mode);
        if current == last {
            Systick::delay(16.millis()).await;
            continue;
        }
        last = current;

        const S_HOLD: u32 = 20;

        let (a_px, d_px, s_px, r_px): (u32, u32, u32, u32) = {
            let total = match mode {
                0 => a_dur + d_dur,
                1 => a_dur + S_HOLD + r_dur,
                _ => a_dur + d_dur + S_HOLD + r_dur,
            }
            .max(1);
            match mode {
                0 => {
                    let a = a_dur * 127 / total;
                    (a, 127 - a, 0, 0)
                }
                1 => {
                    let a = a_dur * 127 / total;
                    let s = S_HOLD * 127 / total;
                    (a, 0, s, 127 - a - s)
                }
                _ => {
                    let a = a_dur * 127 / total;
                    let d = d_dur * 127 / total;
                    let s = S_HOLD * 127 / total;
                    (a, d, s, 127 - a - d - s)
                }
            }
        };

        oled.clear();

        let mode_str = match mode {
            0 => "AD",
            1 => "AR",
            _ => "ADSR",
        };
        Text::new(mode_str, Point::new(0, 7), text_style)
            .draw(&mut oled)
            .ok();

        const TOP: i32 = 9;
        const BOT: i32 = 63;
        const H: i32 = BOT - TOP;

        let level_at = |x: u32| -> f32 {
            match mode {
                0 => {
                    if x < a_px {
                        env_curve(x as f32 / a_px.max(1) as f32, a_crv)
                    } else {
                        1.0 - env_curve((x - a_px) as f32 / d_px.max(1) as f32, d_crv)
                    }
                }
                1 => {
                    if x < a_px {
                        env_curve(x as f32 / a_px.max(1) as f32, a_crv)
                    } else if x < a_px + s_px {
                        1.0
                    } else {
                        1.0 - env_curve((x - a_px - s_px) as f32 / r_px.max(1) as f32, r_crv)
                    }
                }
                _ => {
                    if x < a_px {
                        env_curve(x as f32 / a_px.max(1) as f32, a_crv)
                    } else if x < a_px + d_px {
                        1.0 - env_curve((x - a_px) as f32 / d_px.max(1) as f32, d_crv)
                            * (1.0 - s_val)
                    } else if x < a_px + d_px + s_px {
                        s_val
                    } else {
                        s_val
                            * (1.0
                                - env_curve(
                                    (x - a_px - d_px - s_px) as f32 / r_px.max(1) as f32,
                                    r_crv,
                                ))
                    }
                }
            }
        };

        let to_y = |lv: f32| BOT - (lv.clamp(0.0, 1.0) * H as f32) as i32;

        let mut prev_y = to_y(level_at(0));
        for x in 1u32..128 {
            let y = to_y(level_at(x));
            Line::new(Point::new(x as i32 - 1, prev_y), Point::new(x as i32, y))
                .into_styled(line_style)
                .draw(&mut oled)
                .ok();
            prev_y = y;
        }

        oled.flush().ok();
        Systick::delay(16.millis()).await;
    }
}

fn env_curve(progress: f32, curve: u8) -> f32 {
    let exp = libm::powf(2.0, (curve as f32 - 64.0) / 32.0);
    libm::powf(progress.clamp(0.0, 1.0), exp)
}
