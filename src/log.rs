use crate::config::{Config, LogLevel};
use tracing_subscriber::{fmt::format, filter, reload, prelude::*};

pub fn setup_trace(config: &Config) {
    let (level_filter, level_str) = match config.log_level {
        LogLevel::Trace => (filter::LevelFilter::TRACE, "trace"),
        LogLevel::Debug => (filter::LevelFilter::DEBUG, "debug"),
        LogLevel::Info  => (filter::LevelFilter::INFO, "info"),
        LogLevel::Warn  => (filter::LevelFilter::WARN, "warn"), 
        LogLevel::Error => (filter::LevelFilter::ERROR, "error"),
    };
    let crate_name = env!("CARGO_PKG_NAME");
    let env_filter = filter::EnvFilter::new(&format!("{crate_name}={level_str}"));

    let (level_filter, _reload_handle) = reload::Layer::new(level_filter);
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