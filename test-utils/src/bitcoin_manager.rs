use std::{fs::File, io::Read};

use anyhow::Result;
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine};
use kld::settings::Settings;
use lightning_block_sync::{http::HttpEndpoint, rpc::RpcClient, BlockSource};

use crate::{
    manager::{Check, Manager},
    ports::get_available_port,
};

const NETWORK: &str = "regtest";

pub struct BitcoinManager {
    manager: Manager,
    pub p2p_port: u16,
    pub rpc_port: u16,
    pub network: String,
}

impl BitcoinManager {
    pub async fn start(&mut self, check: impl Check) -> Result<()> {
        let args = &[
            "-server",
            "-noconnect",
            "-rpcthreads=16",
            &format!("-chain={NETWORK}"),
            &format!("-datadir={}", &self.manager.storage_dir),
            &format!("-port={}", &self.p2p_port.to_string()),
            &format!("-rpcport={}", &self.rpc_port.to_string()),
        ];
        self.manager.start("bitcoind", args, check).await
    }

    pub fn cookie_path(&self) -> String {
        cookie_path(&self.manager)
    }

    pub fn test_bitcoin(output_dir: &str, instance: &str) -> BitcoinManager {
        let p2p_port = get_available_port().unwrap();
        let rpc_port = get_available_port().unwrap();

        let manager = Manager::new(output_dir, "bitcoind", instance);
        BitcoinManager {
            manager,
            p2p_port,
            rpc_port,
            network: NETWORK.to_string(),
        }
    }
}

fn cookie_path(manager: &Manager) -> String {
    let dir = if NETWORK == "mainnet" {
        manager.storage_dir.clone()
    } else {
        format!("{}/{}", manager.storage_dir, NETWORK)
    };
    format!("{dir}/.cookie")
}

pub struct BitcoindCheck(pub Settings);

#[async_trait]
impl Check for BitcoindCheck {
    async fn check(&self) -> bool {
        if let Ok(mut file) = File::open(&self.0.bitcoin_cookie_path) {
            let mut cookie = String::new();
            file.read_to_string(&mut cookie).unwrap();
            let credentials = general_purpose::STANDARD.encode(cookie.as_bytes());
            let http_endpoint = HttpEndpoint::for_host(self.0.bitcoind_rpc_host.clone())
                .with_port(self.0.bitcoind_rpc_port);
            let client = RpcClient::new(&credentials, http_endpoint).unwrap();
            client.get_best_block().await.is_ok()
        } else {
            false
        }
    }
}

#[macro_export]
macro_rules! bitcoin {
    ($settings:expr) => {{
        let mut bitcoind = test_utils::bitcoin_manager::BitcoinManager::test_bitcoin(
            env!("CARGO_TARGET_TMPDIR"),
            &$settings.node_id,
        );
        $settings.bitcoin_network =
            kld::settings::Network::from_str(&bitcoind.network).map_err(|e| anyhow::anyhow!(e))?;
        $settings.bitcoind_rpc_port = bitcoind.rpc_port;
        $settings.bitcoin_cookie_path = bitcoind.cookie_path();
        bitcoind
            .start(test_utils::bitcoin_manager::BitcoindCheck(
                $settings.clone(),
            ))
            .await?;
        bitcoind
    }};
}
