use lightning::util::logger::{Level, Logger};
use log::{logger, LevelFilter, Log, Metadata, MetadataBuilder, Record, SetLoggerError};
use std::process;

#[derive(Default)]
/// A logger instance for logfmt format (https://www.brandur.org/logfmt)
pub struct KndLogger {
    node_id: String,
}

pub fn init(node_id: &str) -> Result<(), SetLoggerError> {
    let logger = KndLogger {
        node_id: node_id.to_string(),
    };
    log::set_boxed_logger(Box::new(logger)).map(|()| log::set_max_level(LevelFilter::Info))
}

impl Log for KndLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let level = record.level().to_string().to_lowercase();
        print!("level={}", level);
        print!(" pid={}", process::id());
        print!(" message=\"{}\"", record.args());
        print!(" target=\"{}\"", record.target());
        print!(" node_id={}", self.node_id);
        println!();
    }

    fn flush(&self) {}
}

#[derive(Default)]
pub struct LightningLogger;

impl Logger for LightningLogger {
    fn log(&self, record: &lightning::util::logger::Record) {
        logger().log(
            &log::RecordBuilder::new()
                .args(record.args)
                .file(Some(record.file))
                .line(Some(record.line))
                .metadata(
                    MetadataBuilder::new()
                        .level(match record.level {
                            Level::Gossip => log::Level::Trace,
                            Level::Trace => log::Level::Trace,
                            Level::Debug => log::Level::Debug,
                            Level::Info => log::Level::Info,
                            Level::Warn => log::Level::Warn,
                            Level::Error => log::Level::Error,
                        })
                        .target(record.module_path)
                        .build(),
                )
                .module_path(Some(record.module_path))
                .build(),
        );
    }
}
