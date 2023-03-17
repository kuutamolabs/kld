use super::{cert_is_atleast_valid_for, openssl, CertRenewPolicy};
use crate::Host;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

fn create_tls_key(ca_key_path: &Path) -> Result<()> {
    let p = ca_key_path.display().to_string();
    let args = ["ecparam", "-genkey", "-name", "secp384r1", "-out", &p];
    openssl(&args).context("Failed to create TLS key")?;
    Ok(())
}

fn round_days(seconds: u64) -> String {
    ((seconds + 43200) / 86400).to_string()
}

fn create_or_update_ca_cert(
    ca_cert_path: &Path,
    ca_key_path: &Path,
    policy: &CertRenewPolicy,
) -> Result<()> {
    if !ca_cert_path.exists() {
        openssl(&[
            "req",
            "-new",
            "-x509",
            "-days",
            &round_days(policy.ca_valid_seconds),
            "-key",
            &ca_key_path.display().to_string(),
            "-out",
            &ca_cert_path.display().to_string(),
            "-subj",
            "/CN=Kld CA",
        ])
        .context("Failed to create CA certificate")?;
    } else if !cert_is_atleast_valid_for(ca_cert_path, policy.ca_renew_seconds) {
        let ca_csr_path = ca_cert_path.with_file_name("ca.csr");
        openssl(&[
            "x509",
            "-x509toreq",
            "-in",
            &ca_cert_path.display().to_string(),
            "-signkey",
            &ca_key_path.display().to_string(),
            "-out",
            &ca_csr_path.display().to_string(),
        ])
        .context("Failed to create CA certificate request")?;
        let new_ca_cert_path = ca_cert_path.with_file_name("new-ca.pem");

        openssl(&[
            "x509",
            "-req",
            "-days",
            &round_days(policy.ca_valid_seconds),
            "-in",
            &ca_csr_path.display().to_string(),
            "-signkey",
            &ca_key_path.display().to_string(),
            "-out",
            &new_ca_cert_path.display().to_string(),
        ])
        .context("Failed to create new CA certificate")?;
        let mut ca_cert = std::fs::read(ca_cert_path)
            .with_context(|| {
                format!(
                    "Failed to read CA certificate from {}",
                    ca_cert_path.display()
                )
            })
            .context("Failed to read CA certificate")?;
        let new_ca_cert = std::fs::read(&new_ca_cert_path)
            .with_context(|| {
                format!(
                    "Failed to read new CA certificate from {}",
                    new_ca_cert_path.display()
                )
            })
            .context("Failed to read new CA certificate")?;
        // Drop expired certificates at some point in future?
        // Probably we more likely to upgrade to a different algorithm in the same time frame.
        ca_cert.extend_from_slice(&new_ca_cert);
        std::fs::write(&new_ca_cert_path, &ca_cert)
            .with_context(|| {
                format!(
                    "Failed to write combined CA certificate to {}",
                    new_ca_cert_path.display()
                )
            })
            .context("Failed to write combined CA certificate")?;
        std::fs::rename(&new_ca_cert_path, ca_cert_path)
            .with_context(|| {
                format!(
                    "Failed to rename combined CA certificate to {}",
                    ca_cert_path.display()
                )
            })
            .context("Failed to rename combined CA certificate")?;
    }
    Ok(())
}

fn create_or_update_cert(
    key_path: &Path,
    cert_path: &Path,
    ca_key_path: &Path,
    ca_cert_path: &Path,
    policy: &CertRenewPolicy,
) -> Result<()> {
    if cert_path.exists() && cert_is_atleast_valid_for(cert_path, policy.cert_renew_seconds) {
        return Ok(());
    }
    let cert_conf = cert_path.with_file_name("cert.conf");
    std::fs::write(
        &cert_conf,
        r#"[req]
req_extensions = v3_req
distinguished_name = req_distinguished_name
[req_distinguished_name]
[ v3_req ]
basicConstraints = CA:FALSE
keyUsage = nonRepudiation, digitalSignature, keyEncipherment
subjectAltName = @alt_names
[alt_names]
DNS.1 = localhost
IP.1 = 127.0.0.1
IP.2 = ::1
"#,
    )?;
    openssl(&[
        "req",
        "-new",
        "-key",
        &key_path.display().to_string(),
        "-out",
        &cert_path.display().to_string(),
        "-config",
        &cert_conf.display().to_string(),
        "-subj",
        "/CN=localhost",
    ])
    .context("Failed to create certificate request")?;
    openssl(&[
        "x509",
        "-req",
        "-days",
        &round_days(policy.cert_valid_seconds),
        "-in",
        &cert_path.display().to_string(),
        "-CA",
        &ca_cert_path.display().to_string(),
        "-CAkey",
        &ca_key_path.display().to_string(),
        "-set_serial",
        "01",
        "-out",
        &cert_path.display().to_string(),
        "-extensions",
        "v3_req",
        "-extfile",
        &cert_conf.display().to_string(),
    ])
    .context("Failed to create certificate")?;
    Ok(())
}

/// Create or update certificates for lightning nodes in given directory.
pub fn create_or_update_lightning_certs(
    cert_dir: &Path,
    hosts: &HashMap<String, Host>,
    renew_policy: &CertRenewPolicy,
) -> Result<()> {
    std::fs::create_dir_all(cert_dir).with_context(|| {
        format!(
            "Failed to create directory for lightning certificates: {}",
            cert_dir.display()
        )
    })?;

    let ca_key_path = cert_dir.join("ca.key");
    let ca_cert_path = cert_dir.join("ca.pem");
    if !ca_key_path.exists() {
        create_tls_key(&ca_key_path).with_context(|| {
            format!(
                "Failed to create key for lightning CA certificate: {}",
                ca_key_path.display()
            )
        })?;
    }
    create_or_update_ca_cert(&ca_cert_path, &ca_key_path, renew_policy).with_context(|| {
        format!(
            "Failed to create lightning CA certificate: {}",
            ca_cert_path.display()
        )
    })?;

    for h in hosts.values() {
        let key_path = cert_dir.join(format!("{}.key", h.name));
        let cert_path = cert_dir.join(format!("{}.pem", h.name));
        if !key_path.exists() {
            create_tls_key(&key_path).with_context(|| {
                format!("Failed to create key for lightning certificate: {}", h.name)
            })?
        }
        create_or_update_cert(
            &key_path,
            &cert_path,
            &ca_key_path,
            &ca_cert_path,
            renew_policy,
        )
        .with_context(|| format!("Failed to create lightning certificate: {}", h.name))?
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse_config;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_create_or_update_lightning_certs() -> Result<()> {
        let dir = tempdir().context("Failed to create temporary directory")?;
        let cert_dir = dir.path().join("certs");
        let config_str = r#"
[global]
flake = "github:myfork/near-staking-knd"

[host_defaults]
public_ssh_keys = [
  '''ssh-ed25519 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA foobar'''
]
ipv4_cidr = 24
ipv6_cidr = 48
ipv4_gateway = "199.127.64.1"
ipv6_gateway = "2605:9880:400::1"

[hosts]
[hosts.kld-00]
nixos_module = "kld-node"
ipv4_address = "199.127.64.2"
ipv6_address = "2605:9880:400::2"
ipv6_cidr = 48

[hosts.db-00]
nixos_module = "kld-node"
ipv4_address = "199.127.64.3"
ipv6_address = "2605:9880:400::3"

[hosts.db-01]
nixos_module = "cockroachdb-node"
ipv4_address = "199.127.64.4"
ipv6_address = "2605:9880:400::4"
"#;
        let config = parse_config(config_str, Path::new("/")).context("Failed to parse config")?;

        create_or_update_lightning_certs(&cert_dir, &config.hosts, &CertRenewPolicy::default())
            .context("Failed to create lightning certificates")?;

        let ca_key_path = cert_dir.join("ca.key");
        let ca_cert_path = cert_dir.join("ca.pem");
        let kld_key_path = cert_dir.join("kld-00.key");
        let kld_cert_path = cert_dir.join("kld-00.pem");
        let db0_cert_path = cert_dir.join("db-00.pem");
        let db1_cert_path = cert_dir.join("db-01.pem");

        let certs = vec![
            &ca_cert_path,
            &kld_cert_path,
            &db0_cert_path,
            &db1_cert_path,
        ];
        for c in certs {
            let cert = fs::read_to_string(c)
                .with_context(|| format!("Failed to read certificate: {}", c.display()))?;
            assert!(cert.contains("BEGIN CERTIFICATE"));
            assert!(cert.contains("END CERTIFICATE"));
        }
        let ca_key_modification_time = fs::metadata(&ca_key_path)?.modified()?;
        let ca_cert_modification_time = fs::metadata(&ca_cert_path)?.modified()?;
        let kld_key_modification_time = fs::metadata(&kld_key_path)?.modified()?;

        fs::remove_file(&kld_key_path)?;

        // check if the comand is idempotent
        create_or_update_lightning_certs(&cert_dir, &config.hosts, &CertRenewPolicy::default())?;

        assert_eq!(
            ca_key_modification_time,
            fs::metadata(&ca_key_path)?.modified()?
        );
        assert_eq!(
            ca_cert_modification_time,
            fs::metadata(&ca_cert_path)?.modified()?
        );
        assert_ne!(
            kld_key_modification_time,
            fs::metadata(&kld_key_path)?.modified()?
        );

        let mut renew_now = CertRenewPolicy::default();
        renew_now.ca_renew_seconds = renew_now.ca_valid_seconds + 1;
        renew_now.cert_renew_seconds = renew_now.cert_valid_seconds + 1;

        create_or_update_lightning_certs(&cert_dir, &config.hosts, &renew_now)?;
        assert_ne!(
            ca_cert_modification_time,
            fs::metadata(&ca_cert_path)?.modified()?
        );

        Ok(())
    }
}
