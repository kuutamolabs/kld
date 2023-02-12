pub mod bitcoin_manager;
pub mod cockroach_manager;
pub mod knd_manager;
mod manager;
pub mod ports;
pub mod teos_manager;

use std::{
    fs::{self, File},
    io::Read,
    str::FromStr,
};

use anyhow::{anyhow, Context, Result};
use bitcoin::secp256k1::{PublicKey, SecretKey};
use bitcoin_manager::BitcoinManager;
use clap::{builder::OsStr, Parser};
pub use cockroach_manager::CockroachManager;
use reqwest::{Certificate, Client};
use settings::{Network, Settings};
use tokio_postgres::NoTls;

pub struct TestSettingsBuilder {
    settings: Settings,
}

impl TestSettingsBuilder {
    pub fn new() -> TestSettingsBuilder {
        let mut settings = Settings::parse_from::<Vec<OsStr>, OsStr>(vec![]);
        settings.certs_dir = format!("{}/certs", env!("CARGO_MANIFEST_DIR"));
        TestSettingsBuilder { settings }
    }

    pub fn with_bitcoind(mut self, bitcoind: &BitcoinManager) -> Result<TestSettingsBuilder> {
        self.settings.bitcoin_network =
            Network::from_str(&bitcoind.network).map_err(|e| anyhow!(e))?;
        self.settings.bitcoind_rpc_port = bitcoind.rpc_port;
        self.settings.bitcoin_cookie_path = bitcoind.cookie_path();
        Ok(self)
    }

    pub fn with_database_port(mut self, port: u16) -> TestSettingsBuilder {
        self.settings.database_port = port.to_string();
        self
    }

    pub fn with_data_dir(mut self, data_dir: &str) -> TestSettingsBuilder {
        fs::create_dir_all(data_dir).unwrap();
        self.settings.data_dir = data_dir.to_string();
        self
    }

    pub fn build(self) -> Settings {
        self.settings
    }
}

impl Default for TestSettingsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub fn test_settings() -> Settings {
    TestSettingsBuilder::default().build()
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

pub async fn connection(settings: &Settings) -> Result<tokio_postgres::Client> {
    let log_safe_params = format!(
        "host={} port={} user={} dbname={}",
        settings.database_host,
        settings.database_port,
        settings.database_user,
        settings.database_name
    );
    let mut params = log_safe_params.clone();
    if !settings.database_password.is_empty() {
        params = format!("{} password={}", params, settings.database_password);
    }
    let (client, connection) = tokio_postgres::connect(&params, NoTls)
        .await
        .with_context(|| format!("could not connect to database ({})", log_safe_params))?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            println!("Database connection closed: {}", e)
        }
    });
    Ok(client)
}
