use anyhow::{bail, Result};
use serde_derive::Deserialize;
use std::env;

#[derive(Deserialize)]
struct SystemInfo {
    git_sha: String,
    git_commit_date: String,
}

fn read_system_info() -> Result<SystemInfo> {
    if let Ok(content) = std::fs::read_to_string("/etc/system-info.toml") {
        Ok(toml::from_str::<SystemInfo>(&content)?)
    } else {
        bail!("fail to read /etc/system-info.toml")
    }
}

fn bitcoind_version() -> Result<String> {
    let output = std::process::Command::new("bitcoind")
        .args(["--version"])
        .output()?;
    if output.status.success() {
        if let Some(version) = std::str::from_utf8(&output.stdout)?
            .split('\n')
            .next()
            .and_then(|line| line.split("Bitcoin Core version ").nth(1))
        {
            Ok(version.into())
        } else {
            bail!("fail get version from return bitcoind")
        }
    } else {
        bail!("fail to get version from bitcoind")
    }
}
pub fn system_info(inline: bool) -> String {
    let mut info = vec![("kld-version", env!("CARGO_PKG_VERSION").to_string())];

    if let Ok(system_info) = read_system_info() {
        info.push(("git-sha", system_info.git_sha));
        info.push(("git-commit-date", system_info.git_commit_date));
    }

    if let Ok(version) = bitcoind_version() {
        info.push(("bitcoind-version", version));
    };

    if inline {
        let system_info: Vec<String> = info.iter().map(|i| format!("{}={}", i.0, i.1)).collect();
        system_info.join(" ")
    } else {
        let system_info: Vec<String> = info.iter().map(|i| format!("{}: {}", i.0, i.1)).collect();
        system_info.join("\n")
    }
}
