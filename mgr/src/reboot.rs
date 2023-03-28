use anyhow::Result;

use super::ssh;
use super::Host;

pub fn reboot(hosts: &[Host]) -> Result<()> {
    ssh(hosts, &["nohup reboot &>/dev/null & exit"])
}
