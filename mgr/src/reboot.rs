use anyhow::Result;
use std::path::PathBuf;

use super::ssh;
use super::Host;
use log::info;

pub fn reboot(hosts: &[Host], disk_encryption_key: Option<PathBuf>) -> Result<()> {
    ssh(hosts, &["nohup reboot &>/dev/null & exit"])?;
    info!("Note unlock is required after node reboot");
    Ok(())
}
