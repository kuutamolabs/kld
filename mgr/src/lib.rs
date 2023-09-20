//! A module for deploying and updating nixos-based nodes.

pub use config::{calculate_hash, load_configuration, Config, Host};
pub use flake::{generate_nixos_flake, NixosFlake};
pub use generate_config::generate_config;
pub use install::install;
pub use nixos_rebuild::nixos_rebuild;
pub use reboot::reboot;
pub use ssh::ssh;

pub mod certs;
mod command;
pub mod config;
mod flake;
mod generate_config;
mod install;
pub mod logging;
mod nixos_rebuild;
mod reboot;
pub mod secrets;
pub mod ssh;

/// utils for deploy and control remote machines
pub mod utils;
