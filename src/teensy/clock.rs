//! Teensy SysTick-backed runtime clock.

use rtic_monotonics::Monotonic;

use crate::core::Clock;

/// Millisecond clock backed by `rtic-monotonics` SysTick.
pub struct SystickClock;

impl Clock for SystickClock {
    fn now_ms() -> u32 {
        rtic_monotonics::systick::Systick::now()
            .duration_since_epoch()
            .to_millis()
    }

    async fn delay_ms(ms: u32) {
        use rtic_monotonics::systick::ExtU32;
        rtic_monotonics::systick::Systick::delay(ms.millis()).await
    }
}
