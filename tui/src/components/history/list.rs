use color_eyre::eyre::{eyre, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use crate::action::Action;
use crate::components::{Component, Frame};
use crate::utils::{ts_to_string, WORD_BINDINGS};

pub struct HistoryList {
    command_tx: Option<UnboundedSender<Action>>,
    display: bool,
    /// The index of items selected
    selected: Option<usize>,
    /// List item and parent
    items: Vec<(u64, String)>,
    pool: Pool<SqliteConnectionManager>,
}

impl HistoryList {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self {
            command_tx: None,
            display: false,
            selected: None,
            items: Vec::new(),
            pool,
        }
    }
}

struct SqlResult {
    timestamp: u64,
    command: String,
}

impl HistoryList {
    fn nav_up(&mut self) -> Result<Option<Action>> {
        while self.display && self.selected.is_some() {
            if self.selected == Some(0) {
                self.selected = Some(self.items.len() - 1);
            } else if let Some(idx) = self.selected {
                self.selected = Some(idx - 1);
            }
            if let Some(idx) = self.selected {
                return Ok(Some(Action::History(self.items[idx].0)));
            }
        }
        Ok(None)
    }

    fn nav_down(&mut self) -> Result<Option<Action>> {
        while self.display && self.selected.is_some() {
            if self.selected == Some(self.items.len() - 1) {
                self.selected = Some(0);
            } else if let Some(idx) = self.selected {
                self.selected = Some(idx + 1);
            }
            if let Some(idx) = self.selected {
                return Ok(Some(Action::History(self.items[idx].0)));
            }
        }
        Ok(None)
    }
    fn load_items(&self) -> Result<Vec<(u64, String)>> {
        if let Ok(conn) = self.pool.get() {
            let mut stmt =
                conn.prepare("SELECT timestamp, command FROM history ORDER BY timestamp DESC")?;
            let mut items = Vec::new();
            let rows = stmt.query_map([], |row| {
                Ok(SqlResult {
                    timestamp: row.get(0)?,
                    command: row.get(1)?,
                })
            })?;
            for sql_result in rows.flatten() {
                items.push((sql_result.timestamp, sql_result.command));
            }
            return Ok(items);
        }
        Err(eyre!("Fail to connect DB"))
    }

    fn switch_tab(&mut self, t: &'static str) -> Result<Option<Action>> {
        self.display = t == WORD_BINDINGS.get("History");
        if self.display {
            let mut retry = 0;
            while retry < 3 {
                if let Ok(items) = self.load_items() {
                    self.items = items;
                    if self.items.is_empty() {
                        self.selected = None;
                    } else {
                        self.selected = Some(0);
                    }
                    break;
                }
                retry += 1;
            }
        }
        if self.items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(Action::History(self.items[0].0)))
        }
    }
}

impl Component for HistoryList {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tab(t) => self.switch_tab(t),
            Action::NavUp => self.nav_up(),
            Action::NavDown => self.nav_down(),
            _ => Ok(None),
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>, _area: Rect) -> Result<()> {
        if self.display {
            let mut size = f.size();
            size.y += 3;
            size.height -= 3;

            let block = Block::default();
            f.render_widget(block, size);
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(45), Constraint::Min(0)])
                .split(size);

            let items: Vec<ListItem> = self
                .items
                .iter()
                .enumerate()
                .map(|(idx, i)| {
                    ListItem::new(format!(
                        "{} - {}",
                        ts_to_string(i.0),
                        WORD_BINDINGS.get(&i.1)
                    ))
                    .style(
                        Style::default()
                            .fg(if Some(idx) == self.selected {
                                Color::White
                            } else {
                                Color::Gray
                            })
                            .add_modifier(if Some(idx) == self.selected {
                                Modifier::BOLD
                            } else {
                                Modifier::DIM
                            }),
                    )
                })
                .collect();

            let items = List::new(items).block(Block::default().borders(Borders::ALL));

            f.render_widget(items, chunks[0]);
        }
        Ok(())
    }
}
