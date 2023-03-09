//! A logging module for commandline usage

use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};

#[derive(Default)]
/// A logger instance for logfmt format (https://www.brandur.org/logfmt)
pub struct LogFmtLogger {}

/// Sets global logger.
///
/// # Errors
///
/// An error is returned if a logger has already been set.
///
pub fn init() -> Result<(), SetLoggerError> {
    let logger = Box::new(LogFmtLogger {});
    log::set_boxed_logger(logger).map(|()| log::set_max_level(LevelFilter::Info))
}

impl Log for LogFmtLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let level = record.level();
        let level_name = level.to_string().to_lowercase();

        println!("[{}] {}", level_name, record.args());
    }

    fn flush(&self) {}
}
