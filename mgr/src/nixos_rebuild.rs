use std::path::Path;
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
    secrets_dir: &Path,
    collect_garbage: bool,
) -> Result<()> {
    let secrets = host.secrets(secrets_dir)?;
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
        (if cfg!(target_os = "macos") {
            &target
        } else {
            ""
        }),
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

    let target = host.deploy_ssh_target();
    let args = match host.nixos_module.as_str() {
        "kld-node" => vec![
            target,
            "--".into(),
            "systemd-run".into(),
            "--collect".into(),
            "--unit nixos-upgrade".into(),
            "echo".into(),
            "level=info".into(),
            "$(".into(),
            "kld-cli".into(),
            "message=kld-node-updated".into(),
            "system-info".into(),
            "--inline".into(),
            ")".into(),
        ],
        _node => vec![
            target,
            "--".into(),
            "systemd-run".into(),
            "--collect".into(),
            "--unit nixos-upgrade".into(),
            "echo".into(),
            "level=info".into(),
            format!("message={}-updated", _node),
        ],
    };

    let output = Command::new("ssh").args(&args).output()?;
    if !output.status.success() {
        warn!(
            "Fail to send deployment event: {}",
            std::str::from_utf8(&output.stdout).unwrap_or("stdout utf-8 decode error")
        );
    }
    Ok(())
}
