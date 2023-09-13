use crate::cockroach_manager::{create_database, CockroachManager};
use crate::electrs_manager::ElectrsManager;
use crate::https_client;
use crate::ports::get_available_port;
use anyhow::{anyhow, bail, Context, Result};
use kld::settings::Settings;
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::env::set_var;
use std::fs;
use std::marker::PhantomData;
use std::process::{Child, Command, Stdio};
use tempfile::TempDir;
use tokio::time::{sleep_until, Duration, Instant};

pub struct KldManager<'a> {
    process: Child,
    exporter_port: u16,
    rest_port: u16,
    rest_client: reqwest::Client,
    _electrs: PhantomData<&'a ElectrsManager<'a, 'a>>,
}

impl<'a> KldManager<'a> {
    pub async fn new(
        output_dir: &'a TempDir,
        kld_bin: &str,
        cockroach: &CockroachManager<'a>,
        _electrs: &ElectrsManager<'a, 'a>,
        settings: &mut Settings,
    ) -> Result<KldManager<'a>> {
        let exporter_port = get_available_port()?;
        let rest_port = get_available_port()?;
        settings.rest_api_address = format!("127.0.0.1:{rest_port}");
        settings.exporter_address = format!("127.0.0.1:{exporter_port}");
        settings.peer_port = get_available_port()?;
        let storage_dir = output_dir.path().join(format!("kld_{}", settings.node_id));
        std::fs::create_dir(&storage_dir)?;

        let certs_dir = format!("{}/certs", env!("CARGO_MANIFEST_DIR"));

        create_database(settings).await;

        set_var("KLD_DATA_DIR", &storage_dir);
        set_var("KLD_CERTS_DIR", &certs_dir);
        set_var(
            "KLD_MNEMONIC_PATH",
            storage_dir
                .join("mnemonic")
                .into_os_string()
                .into_string()
                .expect("should not use non UTF-8 code in the path"),
        );
        set_var(
            "KLD_WALLET_NAME",
            format!("kld-wallet-{}", &settings.node_id),
        );
        set_var("KLD_PEER_PORT", settings.peer_port.to_string());
        set_var("KLD_EXPORTER_ADDRESS", &settings.exporter_address);
        set_var("KLD_REST_API_ADDRESS", &settings.rest_api_address);
        set_var("KLD_BITCOIN_NETWORK", settings.bitcoin_network.to_string());
        set_var("KLD_BITCOIN_COOKIE_PATH", &settings.bitcoin_cookie_path);
        set_var("KLD_BITCOIN_RPC_HOST", "127.0.0.1");
        set_var(
            "KLD_BITCOIN_RPC_PORT",
            settings.bitcoind_rpc_port.to_string(),
        );
        set_var("KLD_DATABASE_PORT", cockroach.sql_port.to_string());
        set_var("KLD_DATABASE_NAME", settings.database_name.clone());
        set_var("KLD_NODE_ID", settings.node_id.clone());
        set_var(
            "KLD_DATABASE_CA_CERT_PATH",
            format!("{certs_dir}/cockroach/ca.crt"),
        );
        set_var(
            "KLD_DATABASE_CLIENT_KEY_PATH",
            format!("{certs_dir}/cockroach/client.root.key"),
        );
        set_var(
            "KLD_DATABASE_CLIENT_CERT_PATH",
            format!("{certs_dir}/cockroach/client.root.crt"),
        );
        set_var("KLD_LOG_LEVEL", "debug");
        set_var("KLD_NODE_ALIAS", "kld-00-alias");
        set_var("KLD_ELECTRS_URL", settings.electrs_url.clone());

        let mut process = if std::env::var("KEEP_TEST_ARTIFACTS_IN").is_ok() {
            Command::new(kld_bin)
                .stdout(fs::File::create(storage_dir.join("kld.log"))?)
                .stderr(Stdio::null())
                .spawn()
                .with_context(|| "failed to start kld")?
        } else {
            Command::new(kld_bin)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .with_context(|| "failed to start kld")?
        };

        let macaroon_path = storage_dir.join("macaroons").join("admin.macaroon");

        // Check macaroon for rest api and exporter api directly
        let mut count = 0;
        while !macaroon_path.exists()
            || reqwest::get(format!("http://127.0.0.1:{}/health", exporter_port))
                .await
                .is_err()
        {
            if count > 3 {
                let _ = process.kill();
                bail!("kld fail to initialize");
            } else {
                sleep_until(Instant::now() + Duration::from_secs(1 + count * 3)).await;
                count += 1;
            }
        }

        Ok(KldManager {
            process,
            rest_client: https_client(Some(fs::read(macaroon_path)?))?,
            _electrs: PhantomData,
            exporter_port,
            rest_port,
        })
    }

    pub fn pid(&self) -> u32 {
        self.process.id()
    }

    pub async fn call_exporter(&self, method: &str) -> Result<String, reqwest::Error> {
        reqwest::get(format!(
            "http://127.0.0.1:{}/{}",
            self.exporter_port, method
        ))
        .await?
        .text()
        .await
    }

    pub async fn call_rest_api<T: DeserializeOwned, B: Serialize>(
        &self,
        method: Method,
        route: &str,
        body: B,
    ) -> Result<T> {
        let res = self
            .rest_client
            .request(
                method,
                format!("https://127.0.0.1:{}{}", self.rest_port, route),
            )
            .body(serde_json::to_string(&body).unwrap())
            .send()
            .await?;
        let status = res.status();
        let text = res.text().await?;
        match serde_json::from_str::<T>(&text) {
            Ok(t) => {
                println!("API result: {text}");
                Ok(t)
            }
            Err(e) => {
                println!("Error from API: {status} {text}");
                Err(anyhow!(e))
            }
        }
    }
}

impl Drop for KldManager<'_> {
    fn drop(&mut self) {
        // Report unexpected kld close, try kill the kld process and wait the log
        match self.process.try_wait() {
            Ok(Some(status)) => eprintln!("kld exited unexpected, status code: {status}"),
            Ok(None) => self.process.kill().expect("kld couldn't be killed"),
            Err(_) => panic!("error attempting bitcoind status"),
        }
    }
}
