use std::{fs, os::unix::prelude::PermissionsExt};

use crate::{
    manager::{Check, Manager},
    ports::get_available_port,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use kld::settings::Settings;
use openssl::ssl::{SslConnector, SslFiletype, SslMethod};
use postgres_openssl::MakeTlsConnector;
use tokio_postgres::Client;

pub struct CockroachManager {
    manager: Manager,
    port: u16,
    pub sql_port: u16,
    http_address: String,
    certs_dir: String,
}

impl CockroachManager {
    pub async fn start(&mut self, check: impl Check) -> Result<()> {
        // Cockroach requires certs to be only read/writable by user in secure mode. Git does not track this.
        for file in fs::read_dir(&self.certs_dir)? {
            let file = file?;
            let mut perms = fs::metadata(file.path())?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(file.path(), perms)?;
        }
        let args = &[
            "start-single-node",
            &format!("--listen-addr=127.0.0.1:{}", self.port),
            &format!("--sql-addr=127.0.0.1:{}", self.sql_port),
            &format!("--http-addr={}", self.http_address),
            &format!("--store={}", self.manager.storage_dir),
            &format!("--certs-dir={}", self.certs_dir),
        ];
        self.manager.start("cockroach", args, check).await
    }

    pub fn test_cockroach(output_dir: &str, instance: &str) -> CockroachManager {
        let port = get_available_port().expect("Cannot find free node port for cockroach");
        let http_port = get_available_port().expect("Cannot find free http port for cockroach");
        let sql_port = get_available_port().expect("Cannot find free sql port for cockroach");
        let http_address = format!("127.0.0.1:{http_port}");
        let certs_dir = format!("{}/certs/cockroach", env!("CARGO_MANIFEST_DIR"));

        let manager = Manager::new(output_dir, "cockroach", instance);
        CockroachManager {
            manager,
            port,
            sql_port,
            http_address,
            certs_dir,
        }
    }

    pub fn kill(&mut self) {
        self.manager.kill()
    }
}

pub struct CockroachCheck(pub Settings);

#[async_trait]
impl Check for CockroachCheck {
    async fn check(&self) -> bool {
        connection(&self.0).await.is_ok()
    }
}

pub async fn connection(settings: &Settings) -> Result<Client> {
    let log_safe_params = format!(
        "host={} port={} user={} dbname=defaultdb",
        settings.database_host, settings.database_port, settings.database_user,
    );
    let mut builder = SslConnector::builder(SslMethod::tls())?;
    builder.set_ca_file(&settings.database_ca_cert_path)?;
    builder.set_certificate_file(&settings.database_client_cert_path, SslFiletype::PEM)?;
    builder.set_private_key_file(&settings.database_client_key_path, SslFiletype::PEM)?;
    let connector = MakeTlsConnector::new(builder.build());
    let (client, connection) = tokio_postgres::connect(&log_safe_params, connector)
        .await
        .with_context(|| format!("could not connect to database ({log_safe_params})"))?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    Ok(client)
}

pub async fn create_database(settings: &Settings) {
    let client = connection(settings).await.unwrap();
    client
        .execute(
            &format!("CREATE DATABASE IF NOT EXISTS {}", settings.database_name),
            &[],
        )
        .await
        .unwrap();
    println!("Created database {}", &settings.database_name);
}

#[macro_export]
macro_rules! cockroach {
    ($settings:expr) => {{
        let mut cockroach = test_utils::cockroach_manager::CockroachManager::test_cockroach(
            env!("CARGO_TARGET_TMPDIR"),
            &$settings.node_id,
        );
        $settings.database_port = cockroach.sql_port.to_string();
        cockroach
            .start(test_utils::cockroach_manager::CockroachCheck(
                $settings.clone(),
            ))
            .await?;
        cockroach
    }};
}
