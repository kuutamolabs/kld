use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Frame};
use crate::action::Action;
use crate::utils::WORD_BINDINGS;

pub struct TabBar {
    command_tx: Option<UnboundedSender<Action>>,
    titles: [&'static str; 2],
    index: usize,
}

impl Default for TabBar {
    fn default() -> Self {
        Self {
            command_tx: None,
            titles: [
                WORD_BINDINGS.get("Command List"),
                WORD_BINDINGS.get("History"),
            ],
            index: 0,
        }
    }
}

impl TabBar {
    pub fn next(&mut self) {
        self.index = (self.index + 1) % self.titles.len();
    }
}

impl Component for TabBar {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        let next_action = match action {
            Action::SwitchTab => {
                self.next();
                Some(Action::Tab(self.titles[self.index]))
            }
            _ => None,
        };
        Ok(next_action)
    }

    fn draw(&mut self, f: &mut Frame<'_>, _area: Rect) -> Result<()> {
        let size = f.size();
        let block = Block::default();
        f.render_widget(block, size);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(size);

        let tabs = Tabs::new(self.titles.to_vec())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(block::title::Title {
                        content: Line::styled(
                            "🌔kuutamo Tui",
                            Style::default()
                                .bold()
                                .fg(Color::Rgb(110, 44, 247))
                                .bg(Color::White),
                        ),
                        ..Default::default()
                    })
                    .border_type(BorderType::Rounded),
            )
            .select(self.index)
            .highlight_style(
                Style::default()
                    .bold()
                    .add_modifier(style::Modifier::UNDERLINED),
            );
        f.render_widget(tabs, chunks[0]);
        Ok(())
    }
}
