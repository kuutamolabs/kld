use anyhow::{bail, Result};
use std::env;

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
pub fn system_info(inline: bool) {
    let info = if let Ok(version) = bitcoind_version() {
        vec![
            ("kld-version", env!("CARGO_PKG_VERSION").into()),
            ("git-sha", env!("VERGEN_GIT_SHA").into()),
            ("git-commit-date", env!("VERGEN_GIT_COMMIT_DATE").into()),
            ("bitcoind-version", version),
        ]
    } else {
        vec![
            ("kld-version", env!("CARGO_PKG_VERSION").into()),
            ("git-sha", env!("VERGEN_GIT_SHA").into()),
            ("git-commit-date", env!("VERGEN_GIT_COMMIT_DATE").into()),
        ]
    };
    if inline {
        let system_info: Vec<String> = info.iter().map(|i| format!("{}={}", i.0, i.1)).collect();
        println!("{}", system_info.join(" "))
    } else {
        let system_info: Vec<String> = info.iter().map(|i| format!("{}: {}", i.0, i.1)).collect();
        println!("{}", system_info.join("\n"))
    }
}
// └─[$] <git:(update-message*)> bitcoind --version
// Bitcoin Core version v24.0.1
