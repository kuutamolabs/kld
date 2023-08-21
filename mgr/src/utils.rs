//! utils for deploy and control remote machines
use anyhow::{anyhow, Context, Result};
use std::fs::File;
use std::io::BufReader;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use super::Host;

/// execute remote ssh
pub fn timeout_ssh(host: &Host, command: &[&str], learn_known_host_key: bool) -> Result<Output> {
    let target = host.deploy_ssh_target();
    let mut args = vec!["-o", "ConnectTimeout=10", "-o", "StrictHostKeyChecking=no"];
    if !learn_known_host_key {
        args.push("-o");
        args.push("UserKnownHostsFile=/dev/null");
    }
    args.push(&target);
    args.push("--");
    args.extend(command);
    println!("$ ssh {}", args.join(" "));
    let output = Command::new("ssh")
        .args(args)
        .output()
        .context("Failed to run ssh...")?;
    Ok(output)
}

/// luks unlock via ssh
pub fn unlock_over_ssh(host: &Host, key_file: &PathBuf) -> Result<()> {
    if let Ok(result) = timeout_ssh(
        host,
        &[
            "-o",
            "ConnectTimeout=10",
            "-o",
            "StrictHostKeyChecking=no",
            "exit",
        ],
        true,
    ) {
        if result.status.success() {
            // handle a node already unlocked
            println!("{} already unlocked", host.name);
            return Ok(());
        }
    }
    let target = host.deploy_ssh_target();
    let mut args = vec![
        "-p",
        "2222",
        "-o",
        "ConnectTimeout=10",
        "-o",
        "StrictHostKeyChecking=no",
    ];
    args.push(&target);
    args.push("cryptsetup-askpass");
    let key = {
        let key_file = File::open(key_file)?;
        let mut reader = BufReader::new(key_file);
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;
        buffer
    };
    let mut ssh = Command::new("ssh")
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdin = ssh.stdin.take().ok_or(anyhow!("could not pipe stdin"))?;
    let mut stdout = ssh.stdout.take().ok_or(anyhow!("could not pipe stdout"))?;
    if stdin.write_all(key.as_slice()).is_ok() {
        let _ = stdin.write(b"\n")?;
    } else {
        return Err(anyhow!("fail to enter password"));
    }
    println!("$ ssh {}", args.join(" "));

    let mut buf_string = String::new();

    if stdout.read_to_string(&mut buf_string).is_ok() && buf_string.starts_with("Passphrase for") {
        Ok(())
    } else {
        Err(anyhow!("sshd response unepxected"))
    }
}
