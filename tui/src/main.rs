use clap::Parser;
use color_eyre::eyre::Result;
use r2d2_sqlite::SqliteConnectionManager;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Tick rate, i.e. number of ticks per second
    #[clap(short, long, value_name = "FLOAT", default_value_t = 4.0)]
    pub tick_rate: f64,

    /// Frame rate, i.e. number of frames per second
    #[clap(short, long, value_name = "FLOAT", default_value_t = 4.0)]
    pub frame_rate: f64,

    /// Enable debug components in tui
    #[clap(short, long, env = "DEBUG")]
    pub debug: bool,

    /// Enable log
    #[clap(short, long, env = "LOG")]
    pub log: bool,

    /// Setup log file
    #[clap(long, env = "LOG_FILE")]
    pub log_file: Option<PathBuf>,

    /// Enable log
    #[clap(long, env = "LOG_LEVEL")]
    pub log_level: Option<String>,

    /// Setup data folder or the sqllite file to store the command history
    /// If not no folder specified, it will stoe on memory for using the app
    #[clap(long, env = "DATA")]
    pub data: Option<PathBuf>,

    /// The key binding overwrite config, if unset will try `keybinding.toml`
    #[clap(short, long, default_value = "keybinding.toml")]
    pub key_binding_file: PathBuf,

    /// The secrets folder provided after node installed by kld-mgr
    #[clap(short, long, default_value = "secrets", env = "SECRETS")]
    pub secrets: PathBuf,

    /// The url endpoint to the kld node
    #[clap(short, long, default_value = "http://localhost:9234", env = "NODE_URL")]
    pub node_url: url::Url,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let manager = if let Some(data) = cli.data {
        log::info!("Use database at {}", data.display());
        if data.is_dir() {
            SqliteConnectionManager::file(data.join("lightning-tui.db"))
        } else {
            SqliteConnectionManager::file(data)
        }
    } else {
        log::info!("In memory database");
        SqliteConnectionManager::memory()
    };
    let pool = r2d2::Pool::new(manager).expect("Fail to init db connection pool");
    let conn = pool.get().expect("Fail to connect db");
    if conn.execute("CREATE TABLE IF NOT EXISTS history (timestamp INTEGER, command TEXT, input TEXT, output TEXT);", []).is_ok() {
        log::info!("Init database");
    }

    let config = kld_tui::Config {
        tick_rate: cli.tick_rate,
        frame_rate: cli.frame_rate,
        debug: cli.debug,
        log_enable: cli.log_file.is_some() || cli.log_level.is_some() || cli.log,
        log_file: cli.log_file,
        log_level: cli.log_level,
        pool,
        keybindings: Some(cli.key_binding_file),
        node_url: cli.node_url,
        secrets: cli.secrets,
    };
    if let Err(e) = kld_tui::app(config).await {
        Err(e)
    } else {
        Ok(())
    }
}
