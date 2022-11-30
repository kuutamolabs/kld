use lightning::util::logger::{Level, Logger};
use log::{logger, LevelFilter, Log, Metadata, MetadataBuilder, Record};
use once_cell::sync::OnceCell;
use std::{process, sync::Arc};

/// A logger instance for logfmt format (https://www.brandur.org/logfmt)
#[derive(Debug)]
pub struct KndLogger {
    node_id: String,
}

// LDK requires the Arc so may as well be global.
static KND_LOGGER: OnceCell<Arc<KndLogger>> = OnceCell::new();

impl KndLogger {
    pub fn init(node_id: &str, level_filter: LevelFilter) {
        let logger = KND_LOGGER.get_or_init(|| {
            Arc::new(KndLogger {
                node_id: node_id.to_string(),
            })
        });
        // This function gets called multiple times by the tests so ignore the error.
        let _ = log::set_logger(logger).map(|()| log::set_max_level(level_filter));
    }

    pub fn global() -> Arc<KndLogger> {
        KND_LOGGER.get().expect("logger is not initialized").clone()
    }
}

impl Log for KndLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let level = record.level().to_string().to_lowercase();
            print!("level={}", level);
            print!(" pid={}", process::id());
            print!(" message=\"{}\"", record.args());
            print!(" target=\"{}\"", record.target());
            print!(" node_id={}", self.node_id);
            println!();
        }
    }

    fn flush(&self) {}
}

impl Logger for KndLogger {
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

#[test]
pub fn test_log() {
    let node_id = "one";
    KndLogger::init(node_id, LevelFilter::Info);
    assert_eq!(node_id, KndLogger::global().node_id);

    let metadata = MetadataBuilder::new().level(log::Level::Debug).build();
    assert!(!KndLogger::global().enabled(&metadata));

    let metadata = MetadataBuilder::new().level(log::Level::Info).build();
    assert!(KndLogger::global().enabled(&metadata));

    let metadata = MetadataBuilder::new().level(log::Level::Warn).build();
    assert!(KndLogger::global().enabled(&metadata));
}
