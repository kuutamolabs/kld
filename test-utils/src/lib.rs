pub mod bitcoin_manager;
pub mod cockroach_manager;
pub mod knd_manager;
mod manager;

use std::{fs::File, io::Read};

use bitcoin::secp256k1::{PublicKey, SecretKey};
use clap::{builder::OsStr, Parser};
pub use cockroach_manager::CockroachManager;
use reqwest::{Certificate, Client};
use settings::Settings;

pub struct TestSettingsBuilder {
    settings: Settings,
}

impl TestSettingsBuilder {
    pub fn new() -> TestSettingsBuilder {
        let mut settings = Settings::parse_from::<Vec<OsStr>, OsStr>(vec![]);
        settings.certs_dir = format!("{}/certs", env!("CARGO_MANIFEST_DIR"));
        TestSettingsBuilder { settings }
    }

    pub fn with_node_id(mut self, node_id: &str) -> TestSettingsBuilder {
        self.settings.node_id = node_id.to_string();
        self
    }

    pub fn for_database(mut self, database: &CockroachManager) -> TestSettingsBuilder {
        self.settings.database_port = database.port.to_string();
        self.settings.database_name = "test".to_string();
        self
    }

    pub fn with_data_dir(mut self, data_dir: &str) -> TestSettingsBuilder {
        self.settings.data_dir = data_dir.to_string();
        self
    }

    pub fn build(self) -> Settings {
        self.settings
    }
}

pub fn test_settings() -> Settings {
    TestSettingsBuilder::new().build()
}

pub fn test_settings_for_database(database: &CockroachManager) -> Settings {
    TestSettingsBuilder::new().for_database(database).build()
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
    File::open(format!("{}/certs/knd.crt", env!("CARGO_MANIFEST_DIR")))
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
    pub fn write<P: AsRef<Path>, C: AsRef<[u8]>>(_path: P, _contents: C) -> io::Result<()> {
        Ok(())
    }
    pub fn create_dir_all<P: AsRef<Path>>(_path: P) -> io::Result<()> {
        Ok(())
    }
}
