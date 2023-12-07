use color_eyre::eyre::Result;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use crate::action::Action;
use crate::components::{Component, Frame};
use crate::utils::WORD_BINDINGS;

pub struct HistoryDetails {
    command_tx: Option<UnboundedSender<Action>>,
    display: bool,
    input: String,
    output: String,
    pool: Pool<SqliteConnectionManager>,
}

impl HistoryDetails {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self {
            command_tx: None,
            display: false,
            input: String::new(),
            output: String::new(),
            pool,
        }
    }
}

impl Component for HistoryDetails {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tab(t) => self.display = t == WORD_BINDINGS.get("History"),
            Action::History(ts) => {
                if let Ok(conn) = self.pool.get() {
                    let (input, output) = conn
                        .query_row(
                            "SELECT input, output FROM history WHERE timestamp == ?",
                            [ts.to_string()],
                            |row| {
                                Ok((
                                    row.get::<usize, String>(0).unwrap_or_default(),
                                    row.get::<usize, String>(1).unwrap_or_default(),
                                ))
                            },
                        )
                        .unwrap_or(("".into(), "fail to fetch command history from db".into()));
                    self.input = input;
                    self.output = output;
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
            size.x += 45;
            size.width -= 45;
            let mut history = self.input.clone();
            history.push('\n');
            history += &self.output;
            let p = Paragraph::new(history).block(Block::default().borders(Borders::ALL));
            f.render_widget(p, size);
        }
        Ok(())
    }
}
