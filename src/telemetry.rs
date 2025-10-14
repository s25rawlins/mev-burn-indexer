use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize the tracing subscriber for structured logging.
/// 
/// This sets up a subscriber with the specified log level and environment-based
/// filtering. The subscriber outputs structured logs to stdout, which is suitable
/// for both development and production deployment (where logs can be aggregated).
pub fn init_telemetry(log_level: &str) {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}
