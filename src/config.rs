use clap::{ValueEnum, Parser};



#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Config {
    #[clap(value_enum, short, long, ignore_case=true, default_value_t=LogLevel::Error)]
    pub log_level: LogLevel
}

#[derive(ValueEnum, Clone, Debug)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}
