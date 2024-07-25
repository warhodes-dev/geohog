use crate::config::{Config, LogLevel};
use tracing_subscriber::{fmt::format, filter, prelude::*};

pub fn setup_trace(config: &Config) {

    // Message level
    let level_filter = match config.log_level {
        LogLevel::Trace => filter::LevelFilter::TRACE,
        LogLevel::Debug => filter::LevelFilter::DEBUG,
        LogLevel::Info  => filter::LevelFilter::INFO,
        LogLevel::Warn  => filter::LevelFilter::WARN, 
        LogLevel::Error => filter::LevelFilter::ERROR,
    };

    // Crate visibility
    let crate_name = env!("CARGO_PKG_NAME");
    let env_filter = filter::EnvFilter::new(&format!("{crate_name}"));

    // Time and Date
    let time_format = time::macros::format_description!(
        "[hour]:[minute]:[second].[subsecond digits:5]"
    );
    let time_offset = time::UtcOffset::current_local_offset()
        .unwrap_or_else(|_| time::UtcOffset::UTC);
    let timer = tracing_subscriber::fmt::time::OffsetTime::new(time_offset, time_format);

    let formatted_layer = tracing_subscriber::fmt::layer()
        .event_format(format().compact())
        .with_timer(timer)
        .with_thread_ids(true);

    tracing_subscriber::registry()
        .with(level_filter)
        .with(env_filter)
        .with(formatted_layer)
        .init();
}