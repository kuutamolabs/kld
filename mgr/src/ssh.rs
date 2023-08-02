use anyhow::Result;
use std::path::Path;

use super::command::status_to_pretty_err;
use super::Host;

pub fn ssh(hosts: &[Host], command: &[&str]) -> Result<()> {
    for host in hosts {
        let target = host.deploy_ssh_target();
        let mut args = vec![];
        args.push(target.as_str());
        args.push("--");
        args.extend(command);
        let status = std::process::Command::new("ssh").args(&args).status();
        status_to_pretty_err(status, "ssh", &args)?;
    }
    Ok(())
}

pub fn generate_key_pair(key_path: &Path) -> Result<()> {
    let p = key_path.display().to_string();
    let args = ["-N", "", "-f", &p, "-C", "kld-mgr"];
    let status = std::process::Command::new("ssh-keygen").args(args).status();
    status_to_pretty_err(status, "ssh-keygen", &args)?;
    Ok(())
}
