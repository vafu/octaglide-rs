//! Platform-provided time source for animation and input timing.

/// Millisecond clock supplied by the platform runtime.
pub trait Clock {
    /// Current time in milliseconds since startup.
    fn now_ms() -> u32;

    /// Wait for the requested number of milliseconds.
    async fn delay_ms(ms: u32);
}
