use color_eyre::eyre::Result;
use kld::api::payloads::FundChannel;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use ratatui::{prelude::*, widgets::*};
use serde_json::json;
use tokio::sync::mpsc::UnboundedSender;

use crate::action::Action;
use crate::components::command::parsers::parse_channel_details;
use crate::components::command::Cmd;
use crate::components::{Component, Frame};
use crate::keybinding::{KeyBindingHelps, KeyBindings};
use crate::mode::Mode;
use crate::utils::{ts_to_string, WORD_BINDINGS};

pub struct CmdDetails {
    command_tx: Option<UnboundedSender<Action>>,
    display: bool,
    on_focus: Option<usize>,
    /// The command of items selected
    selected_command: Cmd,
    keybindings_help: KeyBindingHelps,
    pool: Pool<SqliteConnectionManager>,
    inputs: Vec<String>,
    /// The index of item in list in query result
    index: usize,
    /// The length of list in query result
    length: usize,
    error_msg: Option<String>,
}

impl CmdDetails {
    pub fn new(keybinding: &KeyBindings, pool: Pool<SqliteConnectionManager>) -> Self {
        Self {
            command_tx: None,
            display: true,
            on_focus: None,
            selected_command: Cmd::AppInfo,
            keybindings_help: keybinding.help_info(),
            pool,
            inputs: Vec::new(),
            index: 0,
            length: 0,
            error_msg: None,
        }
    }
}

struct SqlResult {
    timestamp: u64,
    input: String,
    output: String,
}

pub trait LastQuery {
    /// show the last result of a command or on query prompt
    fn last_result(&self, cmd: Cmd) -> (Option<u64>, String);
}

impl LastQuery for CmdDetails {
    fn last_result(&self, cmd: Cmd) -> (Option<u64>, String) {
        let mut last_result = String::new();
        let last_query_time = if let Ok(conn) = self.pool.get() {
            match conn.query_row(
                &format!("SELECT timestamp, input, output FROM history WHERE command == '{:?}' ORDER BY timestamp DESC LIMIT 1", cmd),
                [],
                |row|
                Ok(SqlResult{
                    timestamp: row.get(0)?,
                    input: row.get(1)?,
                    output: row.get(2)?,
                })){
                Ok(result) => {
                    if result.output.is_empty() {
                        last_result.push_str(WORD_BINDINGS.get("On query, please wait..."));
                        last_result.push('\n');
                    } else {
                        last_result.push_str(&result.input);
                        last_result.push('\n');
                        last_result.push_str(&result.output);
                        last_result.push('\n');
                    }
                    Some(result.timestamp)
                },
                Err(_) => {
                    last_result.push_str(WORD_BINDINGS.get("Can not find out previous query."));
                    last_result.push('\n');
                    None
                },
            }
        } else {
            None
        };
        (last_query_time, last_result)
    }
}

impl Component for CmdDetails {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tab(t) => self.display = t == WORD_BINDINGS.get("Command List"),
            Action::Command(c) => self.selected_command = c,
            Action::NavLeft if self.length > 0 => {
                if self.index == 0 {
                    self.index = self.length - 1;
                } else {
                    self.index -= 1;
                }
            }
            Action::NavRight if self.length > 0 => {
                if self.index == self.length - 1 {
                    self.index = 0;
                } else {
                    self.index += 1;
                }
            }
            Action::EnterCmdMode => {
                self.on_focus = Some(0);
                match self.selected_command {
                    Cmd::NodeInfo => {
                        return Ok(Some(Action::Execute(Cmd::NodeInfo, String::new())))
                    }
                    Cmd::PeerCont => {
                        self.inputs = vec![String::new()];
                    }
                    Cmd::ChanOpen => {
                        self.inputs = vec![String::new(), String::new()];
                    }
                    Cmd::ChanList => {
                        self.index = 0;
                        self.length = 0;
                        return Ok(Some(Action::Execute(Cmd::ChanList, String::new())));
                    }
                    _ => {
                        // Command not implement currently, exit
                        return Ok(Some(Action::ExitCmdMode));
                    }
                }
            }
            Action::ExitCmdMode => self.on_focus = None,
            Action::TriggerExecute => {
                self.error_msg = None;
                let next_action = match self.selected_command {
                    Cmd::PeerCont => Some(Action::Execute(
                        Cmd::PeerCont,
                        json!({
                            "id": self.inputs[0],
                        })
                        .to_string(),
                    )),
                    Cmd::ChanOpen => {
                        let fund_channel = FundChannel {
                            id: self.inputs[0].clone(),
                            satoshis: self.inputs[1].clone(),
                            fee_rate: None,
                            announce: None,
                            min_conf: None,
                            utxos: Vec::new(),
                            push_msat: None,
                            close_to: None,
                            request_amt: None,
                            compact_lease: None,
                        };
                        match serde_json::to_string(&fund_channel) {
                            Ok(payload) => Some(Action::Execute(Cmd::ChanOpen, payload)),
                            Err(e) => {
                                self.error_msg = Some(format!("{e}"));
                                None
                            }
                        }
                    }
                    _ => None,
                };
                self.inputs = Vec::new();
                return Ok(next_action);
            }
            Action::SwitchInputs => {
                if let Some(on_focus) = self.on_focus {
                    let mut new_focus = on_focus + 1;
                    match self.selected_command {
                        Cmd::PeerCont => new_focus = 0,
                        Cmd::ChanOpen if new_focus > 1 => new_focus = 0,
                        _ => {}
                    }
                    self.on_focus = Some(new_focus);
                }
            }
            Action::Input(key_event) => {
                if let Some(on_focus) = self.on_focus {
                    match key_event.code {
                        crossterm::event::KeyCode::Char(c) => {
                            self.inputs[on_focus].push(c);
                        }
                        crossterm::event::KeyCode::Backspace => {
                            self.inputs[on_focus].pop();
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, _area: Rect) -> Result<()> {
        if self.display {
            let mut size = f.size();
            // TabBar offset
            size.y += 3;
            size.height -= 3;
            // CmdList offset
            size.x += 25;
            size.width -= 25;

            match self.selected_command {
                Cmd::AppInfo => self.app_info(f, size),
                Cmd::NodeInfo => self.node_info(f, size),
                Cmd::ChanOpen => self.channel_open(f, size),
                Cmd::ChanList => self.channel_list(f, size),
                Cmd::PeerCont => self.peer_connect(f, size),
                _ => {
                    let text = Text::from(Line::from(
                        WORD_BINDINGS
                            .get("This command is not ready yet, please use kld-cli instead."),
                    ));
                    let unimplement_prompt =
                        Paragraph::new(text).block(Block::default().borders(Borders::ALL));
                    f.render_widget(unimplement_prompt, size);
                }
            }
        }
        Ok(())
    }
}

impl CmdDetails {
    fn app_info(&mut self, f: &mut Frame<'_>, area: Rect) {
        let mut help_info = WORD_BINDINGS.get("Welcome to 🌔kuutamo Tui").to_string();
        help_info += "\n";
        help_info += WORD_BINDINGS.get("Following are the key setting you are using now");
        help_info += "\n\n";
        for (m, keybinding) in &self.keybindings_help {
            help_info.push_str(&format!("[{m:?}]\n"));
            for (action, keybindings) in keybinding {
                help_info.push_str(&format!("    {action:} - {}\n", keybindings.join(",")));
            }
        }
        help_info.push('\n');
        help_info.push_str(
            WORD_BINDINGS.get("If you have any suggestion please let us know on Github."),
        );
        help_info.push('\n');
        help_info.push_str("https://github.com/kuutamolabs/lightning-tui");
        let p = Paragraph::new(help_info).block(Block::default().borders(Borders::ALL));
        f.render_widget(p, area);
    }
    fn append_execute_hint(&self, again: bool, info: &mut String) {
        if again {
            info.push_str(WORD_BINDINGS.get("Press "));
            info.push_str(
                &self
                    .keybindings_help
                    .get(&Mode::Command)
                    .and_then(|kb| kb.get(&Action::TriggerExecute))
                    .map(|kb_list| kb_list.join("/"))
                    .unwrap_or_default(),
            );
            info.push_str(WORD_BINDINGS.get(" to fetch again"));
        } else {
            info.push_str(WORD_BINDINGS.get("Press "));
            info.push_str(
                &self
                    .keybindings_help
                    .get(&Mode::Command)
                    .and_then(|kb| kb.get(&Action::TriggerExecute))
                    .map(|kb_list| kb_list.join("/"))
                    .unwrap_or_default(),
            );
            info.push_str(WORD_BINDINGS.get(" to fetch"));
        }
    }
    fn node_info(&mut self, f: &mut Frame<'_>, area: Rect) {
        let (last_query_time, mut info) = self.last_result(Cmd::NodeInfo);
        self.append_execute_hint(last_query_time.is_some(), &mut info);

        let p = Paragraph::new(info).block(if let Some(last_query_time) = last_query_time {
            Block::default()
                .title(
                    block::title::Title::from(format!(
                        "{}{}",
                        WORD_BINDINGS.get("Query at "),
                        ts_to_string(last_query_time)
                    ))
                    .position(block::title::Position::Top)
                    .alignment(Alignment::Right),
                )
                .borders(Borders::ALL)
        } else {
            Block::default().borders(Borders::ALL)
        });
        f.render_widget(p, area);
    }
    fn draw_intro(&mut self, f: &mut Frame<'_>, area: Rect) {
        let mut info = WORD_BINDINGS.get("Press ").to_string();
        info.push_str(
            &self
                .keybindings_help
                .get(&Mode::Command)
                .and_then(|kb| kb.get(&Action::TriggerExecute))
                .map(|kb_list| kb_list.join("/"))
                .unwrap_or_default(),
        );
        info.push('/');
        info.push_str(
            &self
                .keybindings_help
                .get(&Mode::Command)
                .and_then(|kb| kb.get(&Action::SwitchInputs))
                .map(|kb_list| kb_list.join("/"))
                .unwrap_or_default(),
        );
        info.push_str(WORD_BINDINGS.get(" to input data.  "));
        info.push_str(WORD_BINDINGS.get("Then, press "));
        info.push_str(
            &self
                .keybindings_help
                .get(&Mode::Command)
                .and_then(|kb| kb.get(&Action::TriggerExecute))
                .map(|kb_list| kb_list.join("/"))
                .unwrap_or_default(),
        );
        info.push_str(WORD_BINDINGS.get(" again to execute.  "));
        info += WORD_BINDINGS.get("Press ");
        info.push_str(
            &self
                .keybindings_help
                .get(&Mode::Command)
                .and_then(|kb| kb.get(&Action::ExitCmdMode))
                .map(|kb_list| kb_list.join("/"))
                .unwrap_or_default(),
        );
        info.push_str(WORD_BINDINGS.get(" to cancel inputs.  "));

        let text = Text::from(Line::from(info));
        let help_message = Paragraph::new(text).block(
            Block::default()
                .borders(Borders::ALL)
                .title(WORD_BINDINGS.get("Introduction")),
        );
        f.render_widget(help_message, area);
    }

    fn show_last_result(&mut self, f: &mut Frame<'_>, area: Rect, cmd: Cmd) {
        let (last_query_time, last_result) = self.last_result(cmd);

        let inner =
            Paragraph::new(last_result).block(if let Some(last_query_time) = last_query_time {
                Block::default()
                    .title(
                        block::title::Title::from(format!(
                            "{}{}",
                            WORD_BINDINGS.get("Query at "),
                            ts_to_string(last_query_time)
                        ))
                        .position(block::title::Position::Top)
                        .alignment(Alignment::Right),
                    )
                    .borders(Borders::ALL)
            } else {
                Block::default().borders(Borders::ALL)
            });
        f.render_widget(inner, area);
    }
    fn show_error_msg(&mut self, f: &mut Frame<'_>, area: Rect, msg: String) {
        let inner = Paragraph::new(msg)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().red().bold());
        f.render_widget(inner, area);
    }
    fn peer_connect(&mut self, f: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(area);
        self.draw_intro(f, chunks[0]);
        if let Some(ref err_msg) = self.error_msg {
            self.show_error_msg(f, chunks[2], err_msg.to_string());
        } else {
            self.show_last_result(f, chunks[2], Cmd::ChanOpen);
        }

        let input = Paragraph::new(
            self.inputs
                .first()
                .map(|s| s.to_string())
                .unwrap_or_default(),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if self.on_focus.is_some() {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                })
                .title(WORD_BINDINGS.get("Public Key")),
        );
        f.render_widget(input, chunks[1]);
    }
    fn channel_open(&mut self, f: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(area);
        self.draw_intro(f, chunks[0]);
        if let Some(ref err_msg) = self.error_msg {
            self.show_error_msg(f, chunks[3], err_msg.to_string());
        } else {
            self.show_last_result(f, chunks[3], Cmd::ChanOpen);
        }

        let pk_input = Paragraph::new(
            self.inputs
                .first()
                .map(|s| s.to_string())
                .unwrap_or_default(),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if self.on_focus == Some(0) {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                })
                .title(WORD_BINDINGS.get("Public Key")),
        );
        f.render_widget(pk_input, chunks[1]);

        let amt_input = Paragraph::new(
            self.inputs
                .get(1)
                .map(|s| s.to_string())
                .unwrap_or_default(),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if self.on_focus == Some(1) {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                })
                .title(WORD_BINDINGS.get("Satoshis")),
        );
        f.render_widget(amt_input, chunks[2]);
    }
    fn channel_list(&mut self, f: &mut Frame<'_>, area: Rect) {
        let (last_query_time, info) = self.last_result(Cmd::ChanList);
        if let Some(last_query_time) = last_query_time {
            let block = Block::default()
                .title(
                    block::title::Title::from(format!(
                        "{}{}",
                        WORD_BINDINGS.get("Query at "),
                        ts_to_string(last_query_time)
                    ))
                    .position(block::title::Position::Top)
                    .alignment(Alignment::Right),
                )
                .borders(Borders::ALL);
            match parse_channel_details(&info) {
                Ok(details) => {
                    self.length = details.len();
                    let widths = [Constraint::Length(30), Constraint::Max(f.size().width - 30)];
                    let table = Table::new(details[self.index].clone(), widths)
                        .block(block)
                        .footer(
                            Row::new(vec![
                                Line::raw(String::from("")),
                                Line::raw(format!(
                                    "{} ({}/{})",
                                    WORD_BINDINGS.get("Page"),
                                    self.index + 1,
                                    self.length
                                ))
                                .alignment(Alignment::Right),
                            ])
                            .style(Style::default().bold()),
                        );
                    f.render_widget(table, area);
                }
                Err(e) => {
                    let mut output = WORD_BINDINGS.get("Decode response error:").to_string();
                    output.push('\n');
                    output += &e.to_string();
                    output.push('\n');
                    output += &info;
                    self.append_execute_hint(true, &mut output);
                    f.render_widget(Paragraph::new(output).block(block), area);
                }
            }
        } else {
            let mut output = String::new();
            self.append_execute_hint(false, &mut output);
            f.render_widget(
                Paragraph::new(output).block(Block::default().borders(Borders::ALL)),
                area,
            );
        }
    }
}
