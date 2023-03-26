pub mod bitcoin_manager;
pub mod cockroach_manager;
pub mod kld_manager;
mod manager;
pub mod ports;

use std::{fs::File, io::Read};

use bitcoin::secp256k1::{PublicKey, SecretKey};
pub use bitcoin_manager::BitcoinManager;
pub use cockroach_manager::CockroachManager;
pub use kld_manager::KldManager;
pub use manager::Check;
use reqwest::{Certificate, Client};
use settings::Settings;

pub fn test_settings(tmp_dir: &str, name: &str) -> Settings {
    let mut settings = Settings::default();
    settings.certs_dir = format!("{}/certs", env!("CARGO_MANIFEST_DIR"));
    settings.database_ca_cert_path =
        format!("{}/certs/cockroach/ca.crt", env!("CARGO_MANIFEST_DIR"));
    settings.database_client_cert_path = format!(
        "{}/certs/cockroach/client.root.crt",
        env!("CARGO_MANIFEST_DIR")
    );
    settings.database_client_key_path = format!(
        "{}/certs/cockroach/client.root.key",
        env!("CARGO_MANIFEST_DIR")
    );
    settings.node_id = name.to_string();
    settings.data_dir = format!("{tmp_dir}/test_{name}");
    settings.mnemonic_path = format!("{}/mnemonic", settings.data_dir);
    std::fs::create_dir_all(&settings.data_dir).unwrap();
    settings
}

pub fn random_public_key() -> PublicKey {
    let rand: [u8; 32] = rand::random();
    let secp_ctx = bitcoin::secp256k1::Secp256k1::new();
    let secret_key = &SecretKey::from_slice(&rand).unwrap();
    PublicKey::from_secret_key(&secp_ctx, secret_key)
}

pub fn https_client() -> Client {
    // Rustls does not support IP addresses (hostnames only) so we need to use native tls (openssl). Also turn off SNI as this requires host names as well.
    reqwest::ClientBuilder::new()
        .tls_sni(false)
        .add_root_certificate(test_cert())
        .use_native_tls()
        .build()
        .unwrap()
}

fn test_cert() -> Certificate {
    let mut buf = Vec::new();
    File::open(format!("{}/certs/kld.crt", env!("CARGO_MANIFEST_DIR")))
        .unwrap()
        .read_to_end(&mut buf)
        .unwrap();
    Certificate::from_pem(&buf).unwrap()
}

#[macro_export]
macro_rules! poll {
    ($secs: expr, $func: expr) => {
        let mut ellapsed = 0;
        while ellapsed < $secs {
            if $func {
                break;
            };
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            ellapsed += 1;
        }
        if ellapsed == $secs {
            panic!("Timed out polling for result");
        }
    };
}

pub mod fake_fs {
    use std::{io, path::Path};

    pub fn read<P: AsRef<Path>>(_path: P) -> io::Result<Vec<u8>> {
        Err(io::Error::from(io::ErrorKind::NotFound))
    }
    pub fn read_to_string<P: AsRef<Path>>(_path: P) -> io::Result<String> {
        Err(io::Error::from(io::ErrorKind::NotFound))
    }
    pub fn write<P: AsRef<Path>, C: AsRef<[u8]>>(_path: P, _contents: C) -> io::Result<()> {
        Ok(())
    }
    pub fn create_dir_all<P: AsRef<Path>>(_path: P) -> io::Result<()> {
        Ok(())
    }
}
