use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use super::Cmd;
use crate::action::Action;
use crate::components::{Component, Frame};
use crate::utils::WORD_BINDINGS;

pub struct CmdList {
    command_tx: Option<UnboundedSender<Action>>,
    display: bool,
    /// The index of items selected
    selected: usize,
    /// List item and parent
    items: Vec<(&'static str, Option<Cmd>)>,
}

impl Default for CmdList {
    fn default() -> Self {
        Self {
            command_tx: None,
            selected: 1,
            items: vec![
                (WORD_BINDINGS.get("App"), None),
                (WORD_BINDINGS.get("information"), Some(Cmd::AppInfo)),
                (WORD_BINDINGS.get("Node"), None),
                (WORD_BINDINGS.get("information"), Some(Cmd::NodeInfo)),
                (WORD_BINDINGS.get("fees"), Some(Cmd::NodeFees)),
                (WORD_BINDINGS.get("estimate liquidity"), Some(Cmd::NodeEslq)),
                (WORD_BINDINGS.get("sign"), Some(Cmd::NodeSign)),
                (WORD_BINDINGS.get("list funds"), Some(Cmd::NodeLsfd)),
                (WORD_BINDINGS.get("Network"), None),
                (WORD_BINDINGS.get("list nodes"), Some(Cmd::NetwLsnd)),
                (WORD_BINDINGS.get("fee rates"), Some(Cmd::NetwFeer)),
                (WORD_BINDINGS.get("Peers"), None),
                (WORD_BINDINGS.get("list"), Some(Cmd::PeerList)),
                (WORD_BINDINGS.get("connect"), Some(Cmd::PeerCont)),
                (WORD_BINDINGS.get("disconnect"), Some(Cmd::PeerDisc)),
                (WORD_BINDINGS.get("Payments"), None),
                (WORD_BINDINGS.get("list"), Some(Cmd::PaymList)),
                (WORD_BINDINGS.get("send key"), Some(Cmd::PaymSdky)),
                (WORD_BINDINGS.get("pay invoice"), Some(Cmd::PaymPayi)),
                (WORD_BINDINGS.get("Invoices"), None),
                (WORD_BINDINGS.get("list"), Some(Cmd::InvoList)),
                (WORD_BINDINGS.get("generate"), Some(Cmd::InvoGene)),
                (WORD_BINDINGS.get("decode"), Some(Cmd::InvoDeco)),
                (WORD_BINDINGS.get("Channels"), None),
                (WORD_BINDINGS.get("list"), Some(Cmd::ChanList)),
                (WORD_BINDINGS.get("open"), Some(Cmd::ChanOpen)),
                (WORD_BINDINGS.get("set fee"), Some(Cmd::ChanSetf)),
                (WORD_BINDINGS.get("close"), Some(Cmd::ChanClos)),
                (WORD_BINDINGS.get("history"), Some(Cmd::ChanHist)),
                (WORD_BINDINGS.get("balance"), Some(Cmd::ChanBala)),
                (WORD_BINDINGS.get("list forwards"), Some(Cmd::ChanLsfd)),
            ],
            display: true,
        }
    }
}

impl CmdList {
    fn nav_up(&mut self) -> Result<Option<Action>> {
        while self.display {
            if self.selected == 0 {
                self.selected = self.items.len() - 1;
            } else {
                self.selected -= 1;
            }
            if let Some(cmd) = &self.items[self.selected].1 {
                return Ok(Some(Action::Command(cmd.clone())));
            }
        }
        Ok(None)
    }
    fn nav_down(&mut self) -> Result<Option<Action>> {
        while self.display {
            if self.selected == self.items.len() - 1 {
                self.selected = 0;
            } else {
                self.selected += 1;
            }
            if let Some(cmd) = &self.items[self.selected].1 {
                return Ok(Some(Action::Command(cmd.clone())));
            }
        }
        Ok(None)
    }
}

impl Component for CmdList {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tab(t) => {
                self.display = t == WORD_BINDINGS.get("Command List");
                Ok(None)
            }
            Action::NavUp => self.nav_up(),
            Action::NavDown => self.nav_down(),
            _ => Ok(None),
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>, _area: Rect) -> Result<()> {
        if self.display {
            // TabBar offset
            let mut size = f.size();
            size.y += 3;
            size.height -= 3;

            let block = Block::default();
            f.render_widget(block, size);
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(25), Constraint::Min(0)])
                .split(size);

            let items: Vec<ListItem> = self
                .items
                .iter()
                .enumerate()
                .map(|(idx, i)| {
                    if i.1.is_some() {
                        ListItem::new(format!("  {}", i.0)).style(
                            Style::default()
                                .fg(if idx == self.selected {
                                    Color::White
                                } else {
                                    Color::Gray
                                })
                                .add_modifier(if idx == self.selected {
                                    Modifier::BOLD
                                } else {
                                    Modifier::DIM
                                }),
                        )
                    } else {
                        ListItem::new(i.0).style(Style::default().fg(Color::Gray))
                    }
                })
                .collect();

            let items = List::new(items).block(Block::default().borders(Borders::ALL));

            f.render_widget(items, chunks[0]);
        }
        Ok(())
    }
}
