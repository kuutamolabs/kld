use std::{fs, os::unix::prelude::PermissionsExt};

use crate::ports::get_available_port;
use anyhow::{bail, Context, Result};
use kld::settings::Settings;
use openssl::ssl::{SslConnector, SslFiletype, SslMethod};
use postgres_openssl::MakeTlsConnector;
use std::process::{Child, Command, Stdio};
use tempfile::TempDir;
use tokio::time::{sleep_until, Duration, Instant};
use tokio_postgres::Client;

pub struct CockroachManager<'a> {
    process: Option<Child>,
    _output_dir: &'a TempDir,
    port: u16,
    pub sql_port: u16,
    http_address: String,
    certs_dir: String,
}

impl<'a> CockroachManager<'a> {
    pub async fn new(
        _output_dir: &'a TempDir,
        settings: &mut Settings,
    ) -> Result<CockroachManager<'a>> {
        let port = get_available_port()?;
        let http_port = get_available_port()?;
        let sql_port = get_available_port()?;
        let http_address = format!("127.0.0.1:{http_port}");
        let certs_dir = format!("{}/certs/cockroach", env!("CARGO_MANIFEST_DIR"));

        let mut cockroach = CockroachManager {
            process: None,
            _output_dir,
            port,
            sql_port,
            http_address,
            certs_dir,
        };

        settings.database_port = cockroach.sql_port;
        cockroach.start().await?;

        // Make sure db connection is ready before return manager
        let mut count = 0;
        while let Err(e) = connection(settings).await {
            if count > 3 {
                return Err(e);
            } else {
                sleep_until(Instant::now() + Duration::from_secs(1 + count * 3)).await;
                count += 1;
            }
        }
        Ok(cockroach)
    }

    pub async fn start(&mut self) -> Result<()> {
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
            &format!("--certs-dir={}", self.certs_dir),
            "--insecure",
            "--store=type=mem,size=0.25",
            // NOTE
            // Uncomment it for debugging , there is not good reason always log
            // &format!(r#"--log="{{file-defaults:{{dir:{}}},sinks:{{stderr:{{filter: NONE}}}}}}""#, self._output_dir.path().join("db.log").display())
        ];

        if self.process.is_some() {
            bail!("Should not CockroachManager should start only once")
        }

        self.process = Some(
            Command::new("cockroach")
                .args(args)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .with_context(|| "failed to start cockroach".to_string())?,
        );
        Ok(())
    }
}

impl Drop for CockroachManager<'_> {
    fn drop(&mut self) {
        let process = self
            .process
            .as_mut()
            .expect("CockroachManager should initialize with `new` function");
        match process.try_wait() {
            Ok(Some(status)) => eprintln!("cockroachdb exited unexpected, status code: {status}"),
            Ok(None) => {
                let _ = Command::new("kill").arg(process.id().to_string()).output();
                let mut count = 0;
                while count < 5 {
                    if let Ok(Some(_)) = process.try_wait() {
                        return;
                    }
                    std::thread::sleep(Duration::from_secs(1 + count * 3));
                    count += 1;
                }
                process.kill().expect("cockroachdb couldn't be killed");
            }
            Err(_) => panic!("error attempting cockroachdb status"),
        }
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
}
