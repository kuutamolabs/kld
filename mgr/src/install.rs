use anyhow::{Context, Result};
use lazy_static::lazy_static;
use log::info;
use std::path::Path;
use std::sync::Mutex;
use std::{
    process::Command,
    sync::mpsc::{channel, Receiver, RecvTimeoutError},
    time::Duration,
};

use crate::{command::status_to_pretty_err, utils::timeout_ssh};

use super::{Host, NixosFlake};

lazy_static! {
    static ref CTRL_WAS_PRESSED: Mutex<Receiver<()>> = {
        let (ctrlc_tx, ctrlc_rx) = channel();
        ctrlc::set_handler(move || {
            info!("received ctrl-c!. Stopping program...");
            let _ = ctrlc_tx.send(());
        })
        .expect("Error setting ctrl-C handler");
        Mutex::new(ctrlc_rx)
    };
}

/// Install a Validator on a given machine
pub fn install(
    hosts: &[Host],
    kexec_url: &str,
    flake: &NixosFlake,
    secrets_dir: &Path,
    debug: bool,
    no_reboot: bool,
) -> Result<()> {
    flake.show()?;
    hosts
        .iter()
        .map(|host| {
            info!("Install {}", host.name);
            let connection_string = if host.install_ssh_user.is_empty() {
                host.ssh_hostname.clone()
            } else {
                format!("{}@{}", host.install_ssh_user, host.ssh_hostname)
            };

            let disk_encryption_key = secrets_dir.join("disk_encryption_key");
            let disk_encryption_key_path = format!(
                "./{}",
                disk_encryption_key
                    .to_str()
                    .expect("Fail to get disk_encryption_key path")
            );
            let secrets = host.secrets(secrets_dir).context("Failed to get secrets")?;
            let flake_uri = format!("{}#{}", flake.path().display(), host.name);
            let extra_files = format!("{}", secrets.path().display());
            let mut args = vec![
                "--extra-files",
                &extra_files,
                "--disk-encryption-keys",
                "/var/lib/disk_encryption_key",
                &disk_encryption_key_path,
                "--kexec",
                kexec_url,
                "--flake",
                &flake_uri,
                "--option",
                "accept-flake-config",
                "true",
            ];
            if cfg!(target_os = "macos") {
                args.push("--build-on-remote")
            }
            if debug {
                args.push("--debug");
            }
            if no_reboot {
                args.push("--no-reboot");
            }
            args.push(&connection_string);
            println!("$ nixos-anywhere {}", args.join(" "));
            let status = Command::new("nixos-anywhere").args(&args).status();
            status_to_pretty_err(status, "nixos-anywhere", &args)?;

            if no_reboot {
                return Ok(());
            }

            info!(
                "Installation of {} finished. Waiting for reboot.",
                host.name
            );

            // remove potential old ssh keys before adding new ones...
            let _ = Command::new("ssh-keygen")
                .args(["-R", &host.ssh_hostname])
                .status()
                .context("Failed to run ssh-keygen to remove old keys...")?;

            // Wait for the machine to come back and learn add it's ssh key to our host
            loop {
                if timeout_ssh(host, &["exit", "0"], true)?.status.success() {
                    break;
                }
                if let Ok(chan) = CTRL_WAS_PRESSED.lock() {
                    if !matches!(
                        chan.recv_timeout(Duration::from_millis(500)),
                        Err(RecvTimeoutError::Timeout)
                    ) {
                        break;
                    }
                }
            }

            Ok(())
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(())
}
