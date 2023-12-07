use crate::components::command::Cmd;
use crate::utils::ts_to_string;
use std::fmt;

use serde::{
    de::{self, Deserializer, Visitor},
    Deserialize, Serialize,
};

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize)]
pub enum Action {
    Tick,
    Render,
    Resize(u16, u16),
    Suspend,
    Resume,
    Quit,
    Refresh,
    Error(String),
    Help,
    SwitchTab,
    Tab(&'static str),
    NavUp,
    NavDown,
    Command(Cmd),
    Execute(Cmd, String),
    EnderCmdMode,
    ExitCmdMode,
    SwitchInputs,
    TriggerExecute,
    History(u64),
    Input(crossterm::event::KeyEvent),
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Tick => write!(f, "Tick"),
            Action::Render => write!(f, "Render"),
            Action::Resize(w, h) => write!(f, "Resize({},{})", w, h),
            Action::Suspend => write!(f, "Suspend"),
            Action::Resume => write!(f, "Resume"),
            Action::Quit => write!(f, "Quit"),
            Action::Refresh => write!(f, "Refresh"),
            Action::Error(e) => write!(f, "Error({})", e),
            Action::Help => write!(f, "Help"),
            Action::SwitchTab => write!(f, "SwitchTab"),
            Action::Tab(t) => write!(f, "Tab({})", t),
            Action::NavUp => write!(f, "NavUp"),
            Action::NavDown => write!(f, "NavDown"),
            Action::Command(cmd) => write!(f, "Command({:?})", cmd),
            Action::Execute(cmd, s) => write!(f, "Execute({:?}, {s:})", cmd),
            Action::EnderCmdMode => write!(f, "EnderCmdMode"),
            Action::ExitCmdMode => write!(f, "ExitCmdMode"),
            Action::SwitchInputs => write!(f, "SwitchInputs"),
            Action::TriggerExecute => write!(f, "TriggerExecute"),
            Action::History(ts) => write!(f, "History({})", ts_to_string(*ts)),
            Action::Input(key_event) => write!(f, "Input({:?})", key_event),
        }
    }
}

impl<'de> Deserialize<'de> for Action {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ActionVisitor;

        impl<'de> Visitor<'de> for ActionVisitor {
            type Value = Action;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid string representation of Action")
            }

            fn visit_str<E>(self, value: &str) -> Result<Action, E>
            where
                E: de::Error,
            {
                match value {
                    "Tick" => Ok(Action::Tick),
                    "Render" => Ok(Action::Render),
                    "Suspend" => Ok(Action::Suspend),
                    "Resume" => Ok(Action::Resume),
                    "Quit" => Ok(Action::Quit),
                    "Refresh" => Ok(Action::Refresh),
                    "Help" => Ok(Action::Help),
                    "SwitchTab" => Ok(Action::SwitchTab),
                    "NavUp" => Ok(Action::NavUp),
                    "NavDown" => Ok(Action::NavDown),
                    "EnderCmdMode" => Ok(Action::EnderCmdMode),
                    "ExitCmdMode" => Ok(Action::ExitCmdMode),
                    "SwitchInputs" => Ok(Action::SwitchInputs),
                    "TriggerExecute" => Ok(Action::TriggerExecute),
                    data if data.starts_with("Error(") => {
                        let error_msg = data.trim_start_matches("Error(").trim_end_matches(')');
                        Ok(Action::Error(error_msg.to_string()))
                    }
                    data if data.starts_with("Resize(") => {
                        let parts: Vec<&str> = data
                            .trim_start_matches("Resize(")
                            .trim_end_matches(')')
                            .split(',')
                            .collect();
                        if parts.len() == 2 {
                            let width: u16 = parts[0].trim().parse().map_err(E::custom)?;
                            let height: u16 = parts[1].trim().parse().map_err(E::custom)?;
                            Ok(Action::Resize(width, height))
                        } else {
                            Err(E::custom(format!("Invalid Resize format: {}", value)))
                        }
                    }
                    _ => Err(E::custom(format!("Unknown Action variant: {}", value))),
                }
            }
        }

        deserializer.deserialize_str(ActionVisitor)
    }
}
