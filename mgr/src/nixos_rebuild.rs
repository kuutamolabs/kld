use std::process::Command;

use crate::command;

use super::{Host, NixosFlake};
use anyhow::{Context, Result};
use log::warn;

/// Runs nixos-rebuild on the given host
pub fn nixos_rebuild(
    action: &str,
    host: &Host,
    flake: &NixosFlake,
    collect_garbage: bool,
) -> Result<()> {
    let secrets = host.secrets()?;
    let target = host.deploy_ssh_target();
    secrets
        .upload(&target)
        .context("Failed to upload secrets")?;
    let flake_uri = host.flake_uri(flake);
    let mut args = vec![
        if action == "rollback" {
            "switch"
        } else {
            action
        },
        "--flake",
        &flake_uri,
        "--option",
        "accept-flake-config",
        "true",
        "--target-host",
        &target,
        "--build-host",
        "",
        "--use-substitutes",
        "--fast",
    ];
    if action == "rollback" {
        args.push("--rollback");
    }
    for i in 1..3 {
        println!("$ nixos-rebuild {}", &args.join(" "));
        let status = Command::new("nixos-rebuild").args(&args).status();
        match command::status_to_pretty_err(status, "nixos-rebuild", &args) {
            Ok(_) => break,
            Err(e) => {
                if i == 1 {
                    warn!("{}", e);
                    warn!("Retry...");
                } else {
                    return Err(e);
                }
            }
        };
    }
    if collect_garbage {
        let ssh_args = [
            &target,
            "--",
            "nix-collect-garbage",
            "--delete-older-than",
            "14d",
        ];
        println!("$ ssh {}", ssh_args.join(" "));
        let status = Command::new("ssh").args(ssh_args).status();
        if let Err(e) = command::status_to_pretty_err(status, "ssh", &ssh_args) {
            warn!("garbage collection failed, but continue...: {}", e);
        }
    }
    Ok(())
}
