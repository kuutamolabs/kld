use crate::command::status_to_pretty_err;
use crate::Host;
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

fn cockroach(args: &[&str]) -> Result<()> {
    println!("cockroach {:?}", args);
    let status = Command::new("cockroach").args(args).status();
    status_to_pretty_err(status, "cockroach", args)?;
    Ok(())
}

fn create_ca(certs_dir: &Path) -> Result<()> {
    let ca_key_path = certs_dir.join("ca.key");
    let ca_crt_path = certs_dir.join("ca.crt");

    let ca_key_exists = ca_key_path.exists();

    // create ca key
    if ca_key_exists && ca_crt_path.exists() {
        return Ok(());
    }

    if ca_key_exists {
        let old_key_path = ca_key_path.with_file_name("ca.key.old");
        fs::rename(&ca_key_path, ca_key_path.with_file_name("ca.key.old")).with_context(|| {
            format!(
                "failed to rename old ca key from {} to {}",
                ca_key_path.display(),
                old_key_path.display()
            )
        })?;
    }

    cockroach(&[
        "cert",
        "create-ca",
        "--certs-dir",
        &certs_dir.display().to_string(),
        "--ca-key",
        &ca_key_path.display().to_string(),
        "--lifetime",
        "262800h",
        "--overwrite",
    ])
    .context("failed to create ca key")
}

fn create_client_cert(certs_dir: &Path, username: &str) -> Result<()> {
    let client_key_path = certs_dir.join(format!("client.{}.key", username));
    let client_crt_path = certs_dir.join(format!("client.{}.crt", username));
    if client_key_path.exists() && client_crt_path.exists() {
        return Ok(());
    }

    cockroach(&[
        "cert",
        "create-client",
        username,
        "--certs-dir",
        &certs_dir.display().to_string(),
        "--ca-key",
        &certs_dir.join("ca.key").display().to_string(),
        "--lifetime",
        "262799h",
        "--overwrite",
    ])
    .with_context(|| format!("failed to create client cert for {}", username))
}

fn create_node_cert(certs_dir: &Path, host: &Host) -> Result<()> {
    let node_key_path = certs_dir.join(format!("{}.node.key", host.name));
    let node_crt_path = certs_dir.join(format!("{}.node.crt", host.name));

    if node_key_path.exists() && node_crt_path.exists() {
        return Ok(());
    }
    cockroach(&[
        "cert",
        "create-node",
        &host.name,
        "localhost",
        "--certs-dir",
        &certs_dir.display().to_string(),
        "--ca-key",
        &certs_dir.join("ca.key").display().to_string(),
        "--lifetime",
        "262799h",
        "--overwrite",
    ])
    .with_context(|| format!("failed to create node cert for {}", host.name))?;

    fs::rename(certs_dir.join("node.crt"), node_crt_path)
        .with_context(|| format!("failed to rename node cert for {}", host.name))?;

    fs::rename(certs_dir.join("node.key"), node_key_path)
        .with_context(|| format!("failed to rename node key for {}", host.name))?;

    Ok(())
}

pub fn create_cockroachdb_certs(certs_dir: &Path, hosts: &BTreeMap<String, Host>) -> Result<()> {
    create_ca(certs_dir)?;

    create_client_cert(certs_dir, "root")?;
    create_client_cert(certs_dir, "kld")?;

    for host in hosts.values() {
        create_node_cert(certs_dir, host)?;
    }
    Ok(())
}

#[test]
fn test_create_cockroachdb_certs() -> Result<()> {
    use crate::config::{parse_config, TEST_CONFIG};
    use tempfile::tempdir;

    let dir = tempdir().context("Failed to create temporary directory")?;
    let config = parse_config(TEST_CONFIG, Path::new("/"), false, false)
        .context("Failed to parse config")?;

    create_cockroachdb_certs(dir.path(), &config.hosts).context("Failed to create certs")?;

    let mut expected_files = ([
        "ca.crt",
        "ca.key",
        "client.root.crt",
        "client.root.key",
        "client.kld.crt",
        "client.kld.key",
    ])
    .map(|f| f.to_string())
    .into_iter()
    .collect::<Vec<_>>();

    for host in config.hosts.values() {
        expected_files.push(format!("{}.node.crt", host.name));
        expected_files.push(format!("{}.node.key", host.name));
    }

    for f in &expected_files {
        assert!(dir.path().join(f).exists(), "Expected file {} to exist", f);
    }

    Ok(())
}
