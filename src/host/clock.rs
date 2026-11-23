//! Tokio-backed runtime clock.

use std::time::Instant;

use crate::core::Clock;

static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Millisecond clock backed by `std::time::Instant`.
pub struct TokioClock;

fn start() -> Instant {
    *START.get_or_init(Instant::now)
}

impl Clock for TokioClock {
    fn now_ms() -> u32 {
        start().elapsed().as_millis() as u32
    }

    async fn delay_ms(ms: u32) {
        tokio::time::sleep(std::time::Duration::from_millis(ms as u64)).await;
    }
}
