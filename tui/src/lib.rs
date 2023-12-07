use color_eyre::eyre::Result;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::path::PathBuf;

mod action;
mod app;
mod components;
mod i18n;
mod keybinding;
mod mode;
mod tui;
mod utils;

use app::{App, ConnectionAuth};

pub struct Config {
    pub tick_rate: f64,
    pub frame_rate: f64,
    pub debug: bool,
    pub log_enable: bool,
    pub log_file: Option<PathBuf>,
    pub log_level: Option<String>,
    pub pool: Pool<SqliteConnectionManager>,
    pub keybindings: Option<PathBuf>,
    pub node_url: url::Url,
    pub secrets: PathBuf,
}

pub async fn app(config: Config) -> Result<()> {
    utils::initialize_logging(
        config.log_file.or(if config.log_enable {
            Some(PathBuf::from(format!("{}.log", env!("CARGO_CRATE_NAME"))))
        } else {
            None
        }),
        config.log_level,
    )?;

    utils::initialize_panic_handler()?;

    let mut app = App::new(
        config.keybindings,
        config.debug,
        config.pool,
        ConnectionAuth::new(config.secrets, config.node_url)?,
    )?;
    app.run(config.tick_rate, config.frame_rate).await?;

    Ok(())
}
