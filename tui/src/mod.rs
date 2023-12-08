use color_eyre::eyre::Result;
use std::path::PathBuf;

mod action;
mod app;
mod components;
mod keybinding;
mod mode;
mod style;
mod tui;
mod utils;

use app::App;

pub struct Config {
    pub tick_rate: f64,
    pub frame_rate: f64,
    pub log_enable: bool,
    pub log_file: Option<PathBuf>,
    pub log_level: Option<String>,
    pub data_dir: Option<PathBuf>,
    pub keybindings: Option<PathBuf>,
    pub secrets: PathBuf,
    pub node_url: url::Url,
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

    let mut app = App::new(config.keybindings, config.connection)?;
    app.run(config.tick_rate, config.frame_rate).await?;

    Ok(())
}
