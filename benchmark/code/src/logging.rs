use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::fmt;

pub(crate) fn init() {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::DEBUG.into())
        .from_env_lossy();

    fmt().with_env_filter(filter).init();
}
