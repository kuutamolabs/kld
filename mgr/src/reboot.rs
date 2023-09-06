use anyhow::Result;

use super::ssh;
use super::Host;
use log::info;

pub fn reboot(hosts: &[Host]) -> Result<()> {
    ssh(hosts, &["nohup reboot &>/dev/null & exit"])?;
    info!("Note unlock is required after node reboot");
    Ok(())
}
