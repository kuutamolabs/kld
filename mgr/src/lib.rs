//! A module for deploying and updating nixos-based validators.

pub use config::{load_configuration, Config, Host};
pub use dry_update::dry_update;
pub use flake::{generate_nixos_flake, NixosFlake};
pub use generate_config::generate_config;
pub use install::install;
pub use nixos_rebuild::nixos_rebuild;
pub use reboot::reboot;
pub use rollback::rollback;
pub use ssh::ssh;
pub use update::update;

pub mod certs;
mod command;
mod config;
mod dry_update;
mod flake;
mod generate_config;
mod install;
pub mod logging;
mod nixos_rebuild;
mod reboot;
mod rollback;
mod secrets;
mod ssh;
mod update;

/// utils for deploy and control remote machines
pub mod utils;
