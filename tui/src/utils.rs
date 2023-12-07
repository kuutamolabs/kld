use std::path::PathBuf;

use crate::i18n::SelectedWordBindings;
use chrono::prelude::DateTime;
use chrono::Utc;
use color_eyre::eyre::Result;
use lazy_static::lazy_static;
use tracing::error;
use tracing_error::ErrorLayer;
use tracing_subscriber::{
    self, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, Layer,
};

lazy_static! {
    pub static ref WORD_BINDINGS: SelectedWordBindings = std::env::var("LANG")
        .ok()
        .as_deref()
        .map(SelectedWordBindings::init)
        .unwrap_or_default();
}

pub fn ts_to_string(timestamp: u64) -> String {
    let dt: DateTime<Utc> =
        DateTime::<Utc>::from_timestamp(timestamp as i64, 0).expect("invalid timestamp");
    dt.to_string()
}

pub fn initialize_panic_handler() -> Result<()> {
    let (panic_hook, eyre_hook) = color_eyre::config::HookBuilder::default()
        .panic_section(format!(
            "This is a bug. Consider reporting it at {}",
            env!("CARGO_PKG_REPOSITORY")
        ))
        .capture_span_trace_by_default(false)
        .display_location_section(false)
        .display_env_section(false)
        .into_hooks();
    eyre_hook.install()?;
    std::panic::set_hook(Box::new(move |panic_info| {
        if let Ok(mut t) = crate::tui::Tui::new(None, None) {
            if let Err(r) = t.exit() {
                error!("Unable to exit Terminal: {:?}", r);
            }
        }

        #[cfg(not(debug_assertions))]
        {
            use human_panic::{handle_dump, print_msg, Metadata};
            let meta = Metadata {
                version: env!("CARGO_PKG_VERSION").into(),
                name: env!("CARGO_PKG_NAME").into(),
                authors: env!("CARGO_PKG_AUTHORS").replace(':', ", ").into(),
                homepage: env!("CARGO_PKG_HOMEPAGE").into(),
            };

            let file_path = handle_dump(&meta, panic_info);
            // prints human-panic message
            print_msg(file_path, &meta)
                .expect("human-panic: printing error message to console failed");
            eprintln!("{}", panic_hook.panic_report(panic_info)); // prints color-eyre stack trace to stderr
        }
        let msg = format!("{}", panic_hook.panic_report(panic_info));
        log::error!("Error: {}", strip_ansi_escapes::strip_str(msg));

        std::process::exit(libc::EXIT_FAILURE);
    }));
    Ok(())
}

pub fn initialize_logging(log_file: Option<PathBuf>, log_level: Option<String>) -> Result<()> {
    if let Some(log_file) = log_file {
        if let Some(parent) = log_file.parent() {
            if parent != std::path::Path::new("") {
                std::fs::create_dir_all(parent)?;
            }
        }
        let log_file = std::fs::File::create(&log_file)?;
        std::env::set_var(
            "RUST_LOG",
            format!(
                "{}={}",
                env!("CARGO_CRATE_NAME"),
                log_level.unwrap_or("info".into())
            ),
        );
        let file_subscriber = tracing_subscriber::fmt::layer()
            .with_file(true)
            .with_line_number(true)
            .with_writer(log_file)
            .with_target(false)
            .with_ansi(false)
            .with_filter(tracing_subscriber::filter::EnvFilter::from_default_env());
        tracing_subscriber::registry()
            .with(file_subscriber)
            .with(ErrorLayer::default())
            .init();
    }
    Ok(())
}

/// Similar to the `std::dbg!` macro, but generates `tracing` events rather
/// than printing to stdout.
///
/// By default, the verbosity level for the generated events is `DEBUG`, but
/// this can be customized.
#[macro_export]
macro_rules! trace_dbg {
    (target: $target:expr, level: $level:expr, $ex:expr) => {{
        match $ex {
            value => {
                tracing::event!(target: $target, $level, ?value, stringify!($ex));
                value
            }
        }
    }};
    (level: $level:expr, $ex:expr) => {
        trace_dbg!(target: module_path!(), level: $level, $ex)
    };
    (target: $target:expr, $ex:expr) => {
        trace_dbg!(target: $target, level: tracing::Level::DEBUG, $ex)
    };
    ($ex:expr) => {
        trace_dbg!(level: tracing::Level::DEBUG, $ex)
    };
}
