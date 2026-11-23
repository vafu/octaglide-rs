//! Host simulator entry point.

/// Runs the host application.
pub fn run() -> ! {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("build tokio runtime");
    runtime.block_on(async {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        }
    })
}
