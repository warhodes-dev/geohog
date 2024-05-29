use crate::config::{Config, LogLevel};
use tracing_subscriber::{fmt::format, filter, reload, prelude::*};

pub fn setup_trace(config: &Config) {
    let filter = match config.log_level {
        LogLevel::Trace => filter::LevelFilter::TRACE,
        LogLevel::Debug => filter::LevelFilter::DEBUG,
        LogLevel::Info  => filter::LevelFilter::INFO,
        LogLevel::Warn  => filter::LevelFilter::WARN,
        LogLevel::Error => filter::LevelFilter::ERROR,
    };
    let (filter, reload_handle) = reload::Layer::new(filter);
    let time_format = time::macros::format_description!(
        "[hour]:[minute]:[second].[subsecond digits:5]"
    );
    let time_offset = time::UtcOffset::current_local_offset()
        .unwrap_or_else(|_| time::UtcOffset::UTC);
    let timer = tracing_subscriber::fmt::time::OffsetTime::new(time_offset, time_format);
    let formatted_layer = tracing_subscriber::fmt::layer()
        .event_format(format().compact())
        .with_timer(timer)
        .without_time();
    tracing_subscriber::registry()
        .with(filter)
        .with(formatted_layer)
        .init();
}