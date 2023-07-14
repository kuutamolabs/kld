use crate::bitcoin_manager::BitcoinManager;
use crate::cockroach_manager::{create_database, CockroachManager};
use crate::electrs_manager::ElectrsManager;
use crate::https_client;
use crate::manager::{Check, Manager};
use crate::ports::get_available_port;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bitcoin::secp256k1::serde::de::DeserializeOwned;
use kld::settings::Settings;
use reqwest::header::{HeaderValue, CONTENT_TYPE};
use reqwest::Method;
use serde::Serialize;
use std::env::set_var;
use std::fs;

pub struct KldManager {
    manager: Manager,
    bin_path: String,
    pub exporter_address: String,
    pub rest_api_address: String,
    pub peer_port: u16,
    rest_client: reqwest::Client,
}

impl KldManager {
    pub async fn start(&mut self, check: impl Check) -> Result<()> {
        self.manager.start(&self.bin_path, &[], check).await
    }

    pub fn pid(&self) -> Option<u32> {
        self.manager.process.as_ref().map(|p| p.id())
    }

    pub async fn call_exporter(&self, method: &str) -> Result<String, reqwest::Error> {
        reqwest::get(format!("http://{}/{}", self.exporter_address, method))
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
        let macaroon = fs::read(format!(
            "{}/macaroons/admin.macaroon",
            self.manager.storage_dir
        ))
        .unwrap();

        let res = self
            .rest_client
            .request(
                method,
                format!("https://{}{}", self.rest_api_address, route),
            )
            .header("macaroon", macaroon)
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
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

    pub async fn test_kld(
        output_dir: &str,
        bin_path: &str,
        bitcoin: &BitcoinManager,
        cockroach: &CockroachManager,
        electrs: &ElectrsManager,
        settings: &Settings,
    ) -> KldManager {
        let exporter_address = format!(
            "127.0.0.1:{}",
            get_available_port().expect("Cannot find free port")
        );
        let rest_api_address = format!(
            "127.0.0.1:{}",
            get_available_port().expect("Cannot find free port")
        );
        let peer_port = get_available_port().expect("Cannot find free port");

        let manager = Manager::new(output_dir, "kld", &settings.node_id);

        let certs_dir = format!("{}/certs", env!("CARGO_MANIFEST_DIR"));

        create_database(settings).await;

        set_var("KLD_DATA_DIR", &manager.storage_dir);
        set_var("KLD_CERTS_DIR", &certs_dir);
        set_var(
            "KLD_MNEMONIC_PATH",
            format!("{}/mnemonic", &manager.storage_dir),
        );
        set_var(
            "KLD_WALLET_NAME",
            format!("kld-wallet-{}", &settings.node_id),
        );
        set_var("KLD_PEER_PORT", peer_port.to_string());
        set_var("KLD_EXPORTER_ADDRESS", &exporter_address);
        set_var("KLD_REST_API_ADDRESS", &rest_api_address);
        set_var("KLD_BITCOIN_NETWORK", &bitcoin.network);
        set_var("KLD_BITCOIN_COOKIE_PATH", bitcoin.cookie_path());
        set_var("KLD_BITCOIN_RPC_HOST", "127.0.0.1");
        set_var("KLD_BITCOIN_RPC_PORT", bitcoin.rpc_port.to_string());
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
        set_var("KLD_ELECTRS_URL", electrs.rpc_address.clone());

        let client = https_client();

        KldManager {
            manager,
            bin_path: bin_path.to_string(),
            exporter_address,
            rest_api_address,
            peer_port,
            rest_client: client,
        }
    }
}

pub struct KldCheck(pub Settings);

#[async_trait]
impl Check for KldCheck {
    async fn check(&self) -> bool {
        if let Ok(res) = reqwest::get(format!("http://{}/health", self.0.exporter_address)).await {
            if let Ok(text) = res.text().await {
                if text == "OK" {
                    return true;
                } else {
                    println!("KLD {} health: {text}", self.0.node_id)
                }
            }
        }
        return false;
    }
}

#[macro_export]
macro_rules! kld {
    ($bitcoin:expr, $cockroach:expr, $electrs:expr, $settings:expr) => {{
        let mut kld = test_utils::kld_manager::KldManager::test_kld(
            env!("CARGO_TARGET_TMPDIR"),
            env!("CARGO_BIN_EXE_kld"),
            $bitcoin,
            $cockroach,
            $electrs,
            &$settings,
        )
        .await;
        $settings.rest_api_address = kld.rest_api_address.clone();
        $settings.exporter_address = kld.exporter_address.clone();
        $settings.peer_port = kld.peer_port;
        kld.start(test_utils::kld_manager::KldCheck($settings.clone()))
            .await?;
        kld
    }};
}
