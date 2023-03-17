//! This module contains the implementation of the `openssl` command.

use crate::command::status_to_pretty_err;
use anyhow::Result;
use std::path::Path;
use std::process::Command;

pub use cockroachdb::create_or_update_cockroachdb_certs;
pub use lightning::create_or_update_lightning_certs;

mod cockroachdb;
mod lightning;

/// This function is used to run the `openssl` command.
pub fn openssl(args: &[&str]) -> Result<()> {
    println!("$ openssl {}", args.join(" "));
    let status = Command::new("openssl").args(args).status();
    status_to_pretty_err(status, "openssl", args)?;
    Ok(())
}

/// This functions checks if a certificate is valid for at least `seconds` seconds.
pub fn cert_is_atleast_valid_for(cert_path: &Path, seconds: u64) -> bool {
    let args = [
        "x509",
        "-in",
        &cert_path.display().to_string(),
        "-checkend",
        &seconds.to_string(),
        "-noout",
    ];
    let status = Command::new("openssl").args(args).status();
    if let Ok(status) = status {
        status.success()
    } else {
        false
    }
}

pub struct CertRenewPolicy {
    ca_renew_seconds: u64,
    ca_valid_seconds: u64,
    cert_renew_seconds: u64,
    cert_valid_seconds: u64,
}

impl Default for CertRenewPolicy {
    fn default() -> Self {
        Self {
            // a year
            ca_renew_seconds: 31536000,
            // ten years
            ca_valid_seconds: 315360000,
            // half a year
            cert_renew_seconds: 15768000,
            // a year
            cert_valid_seconds: 31536000,
        }
    }
}
