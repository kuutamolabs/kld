use color_eyre::eyre::{eyre, Result};
use crossterm::event::KeyEvent;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use ratatui::prelude::Rect;
use std::path::PathBuf;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use url::Url;

use crate::{
    action::Action,
    components::{
        command::list::CmdList,
        command::query,
        command::{details::CmdDetails, Cmd},
        debug::DebugComponent,
        history::details::HistoryDetails,
        history::list::HistoryList,
        tab_bar::TabBar,
        Component,
    },
    keybinding::KeyBindings,
    mode::Mode,
    tui,
};

#[derive(Clone)]
pub struct ConnectionAuth {
    pub url: Url,
    pub pem: Vec<u8>,
    pub macaroon: Vec<u8>,
}

impl ConnectionAuth {
    pub fn new(secrets: PathBuf, url: Url) -> Result<Self> {
        let pem_path = secrets.join("lightning").join("ca.pem");
        if !pem_path.exists() {
            log::error!("no pem under {}", secrets.display());
            return Err(eyre!(
                "Could not find pem under secrets, please provide a correct one with `--secrets`"
            ));
        }
        let pem = match std::fs::read(&pem_path) {
            Ok(content) => content,
            Err(e) => {
                log::error!("Could not read pem: {e}");
                return Err(eyre!("Could not read pem"));
            }
        };

        let macaroon_path = secrets.join("admin.macaroon");
        if !macaroon_path.exists() {
            log::error!("no macaroon under {}", secrets.display());
            return Err(eyre!("Could not find macaroon under secrets, please provide a correct one with `--secrets`"));
        }
        let macaroon = match std::fs::read(&macaroon_path) {
            Ok(content) => content,
            Err(e) => {
                log::error!("Could not read macaroon: {e}");
                return Err(eyre!("Could not read macaroon"));
            }
        };
        Ok(Self { pem, url, macaroon })
    }
}

pub struct App {
    pub keybindings: KeyBindings,
    pub components: Vec<Box<dyn Component>>,
    pub should_quit: bool,
    pub should_suspend: bool,
    pub mode: Mode,
    pub last_tick_key_events: Vec<KeyEvent>,
    pool: Pool<SqliteConnectionManager>,
    connection_auth: ConnectionAuth,
}

impl App {
    pub fn new(
        user_keybinding: Option<PathBuf>,
        debug: bool,
        pool: Pool<SqliteConnectionManager>,
        connection_auth: ConnectionAuth,
    ) -> Result<Self> {
        let mode = Mode::Navigate;
        let keybindings = KeyBindings::new(user_keybinding)?;
        Ok(Self {
            components: if debug {
                vec![
                    Box::<TabBar>::default(),
                    Box::<CmdList>::default(),
                    Box::new(CmdDetails::new(&keybindings, pool.clone())),
                    Box::new(HistoryList::new(pool.clone())),
                    Box::new(HistoryDetails::new(pool.clone())),
                    Box::<DebugComponent>::default(),
                ]
            } else {
                vec![
                    Box::<TabBar>::default(),
                    Box::<CmdList>::default(),
                    Box::new(CmdDetails::new(&keybindings, pool.clone())),
                    Box::new(HistoryList::new(pool.clone())),
                    Box::new(HistoryDetails::new(pool.clone())),
                ]
            },
            should_quit: false,
            should_suspend: false,
            keybindings,
            mode,
            last_tick_key_events: Vec::new(),
            pool,
            connection_auth,
        })
    }

    pub async fn run(&mut self, tick_rate: f64, frame_rate: f64) -> Result<()> {
        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        let mut tui = tui::Tui::new(Some(tick_rate), Some(frame_rate))?;
        tui.enter()?;

        for component in self.components.iter_mut() {
            component.register_action_handler(action_tx.clone())?;
        }

        for component in self.components.iter_mut() {
            component.init(tui.size()?)?;
        }

        loop {
            if let Some(e) = tui.next().await {
                match e {
                    tui::Event::Quit => action_tx.send(Action::Quit)?,
                    tui::Event::Tick => action_tx.send(Action::Tick)?,
                    tui::Event::Render => action_tx.send(Action::Render)?,
                    tui::Event::Resize(x, y) => action_tx.send(Action::Resize(x, y))?,
                    tui::Event::Key(key) => {
                        if let Some(keymap) = self.keybindings.get(&self.mode) {
                            if let Some(action) = keymap.get(&vec![key]) {
                                log::info!("Got action: {action:?}");
                                action_tx.send(action.clone())?;
                            } else {
                                // If the key was not handled as a single key action,
                                // then consider it for multi-key combinations.
                                self.last_tick_key_events.push(key);

                                // Check for multi-key combinations
                                if let Some(action) = keymap.get(&self.last_tick_key_events) {
                                    log::info!("Action triggered: {action:?}");
                                    action_tx.send(action.clone())?;
                                }
                            }
                        };
                        if self.mode == Mode::Command {
                            action_tx.send(Action::Input(key))?;
                        }
                    }
                    _ => {}
                }
                for component in self.components.iter_mut() {
                    if let Some(action) = component.handle_events(Some(e.clone()))? {
                        action_tx.send(action)?;
                    }
                }
            }

            while let Ok(action) = action_rx.try_recv() {
                if action != Action::Tick && action != Action::Render {
                    log::debug!("{action:?}");
                }
                match action {
                    Action::Tick => {
                        self.last_tick_key_events.drain(..);
                    }
                    Action::Quit => self.should_quit = true,
                    Action::Suspend => self.should_suspend = true,
                    Action::Resume => self.should_suspend = false,
                    Action::Resize(w, h) => {
                        tui.resize(Rect::new(0, 0, w, h))?;
                        tui.draw(|f| {
                            for component in self.components.iter_mut() {
                                let r = component.draw(f, f.size());
                                if let Err(e) = r {
                                    action_tx
                                        .send(Action::Error(format!("Failed to draw: {:?}", e)))
                                        .unwrap();
                                }
                            }
                        })?;
                    }
                    Action::Render => {
                        tui.draw(|f| {
                            for component in self.components.iter_mut() {
                                let r = component.draw(f, f.size());
                                if let Err(e) = r {
                                    action_tx
                                        .send(Action::Error(format!("Failed to draw: {:?}", e)))
                                        .unwrap();
                                }
                            }
                        })?;
                    }
                    Action::EnderCmdMode => self.mode = Mode::Command,
                    Action::ExitCmdMode => self.mode = Mode::Navigate,
                    Action::Execute(ref cmd, ref input) => {
                        let trigger_time = unix_timestamp();
                        if let Ok(conn) = self.pool.get() {
                            log::info!("{cmd:?} trigger at {trigger_time:}");
                            if let Err(e) = conn.execute("INSERT INTO history(timestamp, command, input, output) VALUES (?, ?, ?, '')", [&trigger_time.to_string(), &format!("{cmd:?}"), input]) {
                                log::error!("Fail to write record: {e:}");
                            }
                        } else {
                            log::error!("Fail to init {cmd:?} trigger at {trigger_time:}");
                        }

                        let pool = self.pool.clone();
                        let auth = self.connection_auth.clone();
                        let uri = cmd.get_uri().unwrap_or_default();
                        let input = input.to_string();
                        match cmd {
                            Cmd::NodeInfo | Cmd::ChanList => {
                                thread::spawn(move || {
                                    log::trace!("query for {trigger_time:}");
                                    let output = query::get(auth, uri);
                                    match pool.get() {
                                        Ok(conn) => {
                                            if let Err(e) = conn.execute("UPDATE history SET output = ? WHERE timestamp == ?;", [&output, &trigger_time.to_string()]) {
                                                log::error!("Fail to update query result for {trigger_time:}: {}", e);
                                            }
                                        }
                                        Err(e) => log::error!(
                                            "Fail to get db connection for {trigger_time:}: {}",
                                            e
                                        ),
                                    }
                                });
                                action_tx.send(Action::ExitCmdMode)?;
                            }
                            Cmd::PeerCont | Cmd::ChanOpen => {
                                thread::spawn(move || {
                                    log::trace!("query for {trigger_time:}");
                                    let output = query::post(auth, uri, input);
                                    match pool.get() {
                                        Ok(conn) => {
                                            if let Err(e) = conn.execute("UPDATE history SET output = ? WHERE timestamp == ?;", [&output, &trigger_time.to_string()]) {
                                                log::error!("Fail to update query result for {trigger_time:}: {}", e);
                                            }
                                        }
                                        Err(e) => log::error!(
                                            "Fail to get db connection for {trigger_time:}: {}",
                                            e
                                        ),
                                    }
                                });
                                action_tx.send(Action::ExitCmdMode)?;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
                for component in self.components.iter_mut() {
                    if let Some(action) = component.update(action.clone())? {
                        action_tx.send(action)?
                    };
                }
            }
            if self.should_suspend {
                let (tick_rate, frame_rate) = (tui.tick_rate, tui.frame_rate);
                tui.suspend()?;
                action_tx.send(Action::Resume)?;
                tui = tui::Tui::new(Some(tick_rate), Some(frame_rate))?;
                tui.enter()?;
            } else if self.should_quit {
                tui.stop()?;
                break;
            }
        }
        tui.exit()?;
        Ok(())
    }
}

fn unix_timestamp() -> u64 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    since_the_epoch.as_secs()
}
