use super::openssl;
use crate::Host;

use anyhow::{Context, Result};
use slice_as_array::{slice_as_array, slice_as_array_transmute};
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::fs;
use std::net::IpAddr;
use std::path::Path;
use std::str::FromStr;
use x509_parser::extensions::GeneralName;
use x509_parser::extensions::ParsedExtension;
use x509_parser::pem::Pem;

fn create_tls_key(ca_key_path: &Path) -> Result<()> {
    let p = ca_key_path.display().to_string();
    let args = ["ecparam", "-genkey", "-name", "secp384r1", "-out", &p];
    openssl(&args).context("Failed to create TLS key")?;
    Ok(())
}

fn create_ca_cert(ca_cert_path: &Path, ca_key_path: &Path) -> Result<()> {
    if !ca_cert_path.exists() {
        openssl(&[
            "req",
            "-new",
            "-x509",
            "-days",
            "10950",
            "-key",
            &ca_key_path.display().to_string(),
            "-out",
            &ca_cert_path.display().to_string(),
            "-subj",
            "/CN=Kld CA",
        ])
        .context("Failed to create CA certificate")?;
    }
    Ok(())
}

fn create_cert(
    cert_dir: &Path,
    ca_key_path: &Path,
    ca_cert_path: &Path,
    host: &Host,
) -> Result<()> {
    let key_path = cert_dir.join(format!("{}.key", host.name));
    let cert_path = cert_dir.join(format!("{}.pem", host.name));
    let current_san = san_from_cert(&cert_path)?;

    let mut has_new_ip = false;
    if let Some(ipv4) = host.ipv4_address {
        if !current_san.contains(&ipv4) && !host.api_ip_access_list.is_empty() {
            has_new_ip = true;
        }
    }
    if let Some(ipv6) = host.ipv4_address {
        if !current_san.contains(&ipv6) && !host.api_ip_access_list.is_empty() {
            has_new_ip = true;
        }
    }

    if !key_path.exists() {
        create_tls_key(&key_path).with_context(|| {
            format!(
                "Failed to create key for lightning certificate: {}",
                host.name
            )
        })?
    }

    if !has_new_ip && cert_path.exists() {
        return Ok(());
    }

    let cert_conf = cert_path.with_file_name("cert.conf");
    let mut conf = r#"[req]
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
"#
    .to_string();
    let mut ip_num = 3;
    if let Some(ip) = host.ipv4_address {
        conf += &format!("IP.{ip_num} = {ip}\n");
        ip_num += 1;
    }
    if let Some(ip) = host.ipv6_address {
        conf += &format!("IP.{ip_num} = {ip}\n");
        ip_num += 1;
    }

    if std::net::IpAddr::from_str(&host.hostname).is_ok() {
        conf += &format!("IP.{ip_num} = {}\n", host.hostname);
    } else {
        conf += &format!("DNS.2 = {}\n", host.hostname);
    }

    std::fs::write(&cert_conf, conf)?;
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
        "10950",
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
pub fn create_lightning_certs(cert_dir: &Path, hosts: &BTreeMap<String, Host>) -> Result<()> {
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
    create_ca_cert(&ca_cert_path, &ca_key_path).with_context(|| {
        format!(
            "Failed to create lightning CA certificate: {}",
            ca_cert_path.display()
        )
    })?;

    for h in hosts.values() {
        create_cert(cert_dir, &ca_key_path, &ca_cert_path, h)
            .with_context(|| format!("Failed to create lightning certificate: {}", h.name))?
    }

    Ok(())
}

/// Parse subject alternative name from certificate
fn san_from_cert(cert_path: &Path) -> Result<HashSet<IpAddr>> {
    let mut sans = HashSet::new();
    if let Ok(data) = fs::read(cert_path) {
        for pem in Pem::iter_from_buffer(&data) {
            let pem = pem?;
            let x509 = pem.parse_x509()?;
            for ext in x509.extensions() {
                if let ParsedExtension::SubjectAlternativeName(san) = ext.parsed_extension() {
                    for name in san.general_names.iter() {
                        if let GeneralName::IPAddress(byte) = name {
                            #[allow(clippy::transmute_ptr_to_ref)]
                            if let Some(ipv4_byte) = slice_as_array!(byte, [u8; 4]) {
                                sans.insert(IpAddr::from(*ipv4_byte));
                            }
                            #[allow(clippy::transmute_ptr_to_ref)]
                            if let Some(ipv6_byte) = slice_as_array!(byte, [u8; 16]) {
                                sans.insert(IpAddr::from(*ipv6_byte));
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(sans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{parse_config, TEST_CONFIG};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_create_lightning_certs() -> Result<()> {
        let dir = tempdir().context("Failed to create temporary directory")?;
        let cert_dir = dir.path().join("certs");

        let config = parse_config(TEST_CONFIG, Path::new("/"), false, false)
            .context("Failed to parse config")?;

        create_lightning_certs(&cert_dir, &config.hosts)
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

        // check if the command is idempotent
        create_lightning_certs(&cert_dir, &config.hosts)?;

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
        Ok(())
    }
}
