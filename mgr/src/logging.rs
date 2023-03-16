//! A logging module for commandline usage

use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};

#[derive(Default)]
/// A logger instance
pub struct Logger {}

static LOGGER: Logger = Logger {};

/// Sets global logger.
///
/// # Errors
///
/// An error is returned if a logger has already been set.
///
pub fn init() -> Result<(), SetLoggerError> {
    log::set_logger(&LOGGER).map(|()| log::set_max_level(LevelFilter::Info))
}

impl Log for Logger {
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
