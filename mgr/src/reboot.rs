use anyhow::Result;
use std::path::PathBuf;

use super::ssh;
use super::Host;
use crate::utils::unlock_over_ssh;

pub fn reboot(hosts: &[Host], disk_encryption_key: Option<PathBuf>) -> Result<()> {
    ssh(hosts, &["nohup reboot &>/dev/null & exit"])?;

    if let Some(disk_encryption_key) = disk_encryption_key {
        for host in hosts {
            loop {
                if unlock_over_ssh(host, &disk_encryption_key).is_ok() {
                    break;
                }
            }
        }
    }

    Ok(())
}
