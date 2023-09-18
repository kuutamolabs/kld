use super::Host;
use crate::utils::{timeout_ssh, unlock_over_ssh};
use anyhow::{bail, Result};
use std::path::PathBuf;

/// Trigger nixos-upgrade to upgrade host on a given machine,
pub fn upgrade(host: &Host, disk_encryption_key: &PathBuf) -> Result<()> {
    // TODO
    // pipe installing log
    if let Ok(output) = std::process::Command::new("ssh")
        .args([
            host.deploy_ssh_target().as_str(),
            "--",
            "systemctl",
            "start",
            "nixos-upgrade",
        ])
        .output()
    {
        if !output.status.success() {
            let error_msg = std::str::from_utf8(&output.stderr).unwrap_or("fail to decode stderr");

            // Node will reboot after upgrade, so the D-Bus connection will terminated under
            // expected
            if !error_msg.starts_with("Warning! D-Bus connection terminated.") {
                bail!(
                    "trigger nixos-upgrade of {} error: {}",
                    host.name,
                    error_msg
                );
            }
        }
    } else {
        bail!("Fail to trigger nixos-upgrade for {}", host.name);
    }
    loop {
        if unlock_over_ssh(host, disk_encryption_key).is_ok() {
            println!("\n# Unlocked {}", host.name);
            break;
        }
    }

    loop {
        // After unlock the sshd will start in port 22
        if timeout_ssh(host, &["exit", "0"], true)?.status.success() {
            break;
        }
    }
    // TODO
    // garbage collect after upgraded
    Ok(())
}
