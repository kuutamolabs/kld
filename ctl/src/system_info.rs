use anyhow::{anyhow, Context, Result};
use serde_derive::Deserialize;
use std::env;

#[derive(Deserialize)]
struct SystemInfo {
    git_sha: String,
    git_commit_date: String,
}

fn read_system_info() -> Result<SystemInfo> {
    let content = std::fs::read_to_string("/etc/system-info.toml")
        .context("fail to read /etc/system-info.toml")?;
    Ok(toml::from_str::<SystemInfo>(&content)?)
}

fn bitcoind_version() -> Result<String> {
    let output = std::process::Command::new("bitcoind")
        .args(["--version"])
        .output()
        .context("could not run bitcoind command")?;
    std::str::from_utf8(&output.stdout)?
        .split('\n')
        .next()
        .and_then(|line| line.split("Bitcoin Core version ").nth(1))
        .ok_or(anyhow!(
            "failed to parse version from bitcoind command output"
        ))
        .map(|version| version.into())
}

fn cockroach_version() -> Result<String> {
    let output = std::process::Command::new("cockroach")
        .args(["version"])
        .output()
        .context("could not run cockroach command")?;
    std::str::from_utf8(&output.stdout)?
        .split('\n')
        .next()
        .and_then(|line| line.split("Build Tag:        ").nth(1))
        .ok_or(anyhow!("failed to parse version from cockroach output"))
        .map(|version| version.into())
}

pub fn system_info(inline: bool) {
    let mut info = vec![("kld-version", env!("CARGO_PKG_VERSION").to_string())];

    if let Ok(system_info) = read_system_info() {
        info.push(("git-sha", system_info.git_sha));
        info.push(("git-commit-date", system_info.git_commit_date));
    }

    if let Ok(version) = bitcoind_version() {
        info.push(("bitcoind-version", version));
    };

    if let Ok(version) = cockroach_version() {
        info.push(("cockroach-version", version));
    };

    if inline {
        let system_info: Vec<String> = info.iter().map(|i| format!("{}={}", i.0, i.1)).collect();
        println!("{}", system_info.join(" "));
    } else {
        let system_info: Vec<String> = info.iter().map(|i| format!("{}: {}", i.0, i.1)).collect();
        println!("{}", system_info.join("\n"));
    }
}
