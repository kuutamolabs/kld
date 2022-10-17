use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};
use std::process;

#[derive(Default)]
/// A logger instance for logfmt format (https://www.brandur.org/logfmt)
pub struct LogFmtLogger {
    node_id: String,
}

/// Sets global logger.
///
/// # Errors
///
/// An error is returned if a logger has already been set.
///
pub fn init(node_id: &str) -> Result<(), SetLoggerError> {
    let logger = Box::new(LogFmtLogger {
        node_id: node_id.to_string(),
    });
    log::set_boxed_logger(logger).map(|()| log::set_max_level(LevelFilter::Info))
}

impl Log for LogFmtLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let level = record.level();
        let level_name = level.to_string().to_lowercase();

        print!("level={}", level_name);
        print!(" pid={}", process::id());
        print!(" message=\"{}\"", record.args());
        print!(" target=\"{}\"", record.target());
        print!(" node_id={}", self.node_id);
        println!();
    }

    fn flush(&self) {}
}
