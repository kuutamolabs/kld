use super::cert_is_atleast_valid_for;
use super::CertRenewPolicy;
use crate::command::status_to_pretty_err;
use crate::Host;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

fn cockroach(args: &[&str]) -> Result<()> {
    println!("cockroach {:?}", args);
    let status = Command::new("cockroach").args(args).status();
    status_to_pretty_err(status, "cockroach", args)?;
    Ok(())
}

fn create_or_update_ca(certs_dir: &Path, policy: &CertRenewPolicy) -> Result<()> {
    let ca_key_path = certs_dir.join("ca.key");
    let ca_crt_path = certs_dir.join("ca.crt");

    let ca_key_exists = ca_key_path.exists();

    // create ca key
    if ca_key_exists
        && ca_crt_path.exists()
        && cert_is_atleast_valid_for(&ca_crt_path, policy.ca_renew_seconds)
    {
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

    // if certificate expires in less than 1 year, create a new one

    cockroach(&[
        "cert",
        "create-ca",
        "--certs-dir",
        &certs_dir.display().to_string(),
        "--ca-key",
        &ca_key_path.display().to_string(),
        "--lifetime",
        &format!("{}s", policy.ca_valid_seconds),
        "--overwrite",
    ])
    .context("failed to create ca key")
}

fn create_or_update_client_cert(
    certs_dir: &Path,
    username: &str,
    policy: &CertRenewPolicy,
) -> Result<()> {
    let client_key_path = certs_dir.join(format!("client.{}.key", username));
    let client_crt_path = certs_dir.join(format!("client.{}.crt", username));
    if client_key_path.exists()
        && client_crt_path.exists()
        && cert_is_atleast_valid_for(&client_crt_path, policy.cert_renew_seconds)
    {
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
        &format!("{}s", policy.cert_valid_seconds),
        "--overwrite",
    ])
    .with_context(|| format!("failed to create client cert for {}", username))
}

fn create_or_update_node_cert(
    certs_dir: &Path,
    host: &Host,
    policy: &CertRenewPolicy,
) -> Result<()> {
    let node_key_path = certs_dir.join(format!("{}.node.key", host.name));
    let node_crt_path = certs_dir.join(format!("{}.node.crt", host.name));

    if node_key_path.exists()
        && node_crt_path.exists()
        && cert_is_atleast_valid_for(&node_crt_path, policy.cert_renew_seconds)
    {
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
        &format!("{}s", policy.cert_valid_seconds),
        "--overwrite",
    ])
    .with_context(|| format!("failed to create node cert for {}", host.name))?;

    fs::rename(certs_dir.join("node.crt"), node_crt_path)
        .with_context(|| format!("failed to rename node cert for {}", host.name))?;

    fs::rename(certs_dir.join("node.key"), node_key_path)
        .with_context(|| format!("failed to rename node key for {}", host.name))?;

    Ok(())
}

pub fn create_or_update_cockroachdb_certs(
    certs_dir: &Path,
    hosts: &HashMap<String, Host>,
    policy: &CertRenewPolicy,
) -> Result<()> {
    create_or_update_ca(certs_dir, policy)?;

    create_or_update_client_cert(certs_dir, "root", policy)?;
    create_or_update_client_cert(certs_dir, "kld", policy)?;

    for host in hosts.values() {
        create_or_update_node_cert(certs_dir, host, policy)?;
    }
    Ok(())
}

#[test]
fn test_create_or_update_cockroachdb_certs() -> Result<()> {
    use crate::config::{parse_config, TEST_CONFIG};
    use tempfile::tempdir;

    let dir = tempdir().context("Failed to create temporary directory")?;
    let config = parse_config(TEST_CONFIG, Path::new("/")).context("Failed to parse config")?;

    create_or_update_cockroachdb_certs(dir.path(), &config.hosts, &CertRenewPolicy::default())
        .context("Failed to create certs")?;

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

    let file_modification_times = expected_files
        .iter()
        .map(|f| {
            let path = dir.path().join(f);
            let metadata = fs::metadata(path).context("Failed to get file metadata")?;
            let modified = metadata
                .modified()
                .context("Failed to get file modification time")?;
            Ok((f.clone(), modified))
        })
        .collect::<Result<HashMap<_, _>>>()?;

    create_or_update_cockroachdb_certs(dir.path(), &config.hosts, &CertRenewPolicy::default())
        .context("Failed to create certs")?;

    for f in &expected_files {
        let path = dir.path().join(f);
        let metadata = fs::metadata(&path).context("Failed to get file metadata")?;
        let modified = file_modification_times
            .get(f)
            .with_context(|| format!("Expected file {} to be in file_modification_times", f))?;
        let modified2 = metadata
            .modified()
            .context("Failed to get file modification time")?;
        assert_eq!(
            *modified, modified2,
            "Expected file {} to not be modified",
            f
        );
    }
    let mut renew_now = CertRenewPolicy::default();
    renew_now.ca_renew_seconds = renew_now.ca_valid_seconds + 1;
    renew_now.cert_renew_seconds = renew_now.cert_valid_seconds + 1;

    create_or_update_cockroachdb_certs(dir.path(), &config.hosts, &renew_now)?;

    for f in &expected_files {
        let path = dir.path().join(f);
        let metadata = fs::metadata(&path).context("Failed to get file metadata")?;
        let modified = file_modification_times.get(f).unwrap();
        let modified2 = metadata
            .modified()
            .context("Failed to get file modification time")?;
        assert_ne!(
            *modified, modified2,
            "Expected file {} to not be modified",
            f
        );
    }

    Ok(())
}
