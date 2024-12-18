use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::fmt;

pub(crate) fn init() {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::DEBUG.into())
        // Set the RUST_LOG env variable to either "info", "warn", or "error"
        // to override the default log level (debug) set above.
        .from_env_lossy();
    fmt().with_env_filter(filter).init();
}
