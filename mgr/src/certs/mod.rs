//! This module contains the implementation of the `openssl` command.

use crate::command::status_to_pretty_err;
use anyhow::Result;
use std::process::Command;

pub use cockroachdb::create_cockroachdb_certs;
pub use lightning::create_lightning_certs;

mod cockroachdb;
mod lightning;

/// This function is used to run the `openssl` command.
pub fn openssl(args: &[&str]) -> Result<()> {
    println!("$ openssl {}", args.join(" "));
    let status = Command::new("openssl").args(args).status();
    status_to_pretty_err(status, "openssl", args)?;
    Ok(())
}
