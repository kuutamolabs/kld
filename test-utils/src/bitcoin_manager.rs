use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use bitcoin::Address;
use kld::bitcoind::BitcoindClient;
use kld::settings::Settings;
use once_cell::sync::OnceCell;

use crate::{
    manager::{Check, Manager},
    ports::get_available_port,
};

pub struct BitcoinManager {
    manager: Manager,
    pub p2p_port: u16,
    pub rpc_port: u16,
    pub network: String,
    pub settings: Settings,
    pub client: OnceCell<BitcoindClient>,
}

impl BitcoinManager {
    pub async fn start(&mut self, check: impl Check) -> Result<()> {
        let args = &[
            "-server",
            "-noconnect",
            "-rpcthreads=16",
            "-listen",
            &format!("-chain={}", &self.network),
            &format!("-datadir={}", &self.manager.storage_dir),
            &format!("-port={}", &self.p2p_port.to_string()),
            &format!("-rpcport={}", &self.rpc_port.to_string()),
        ];
        self.manager.start("bitcoind", args, check).await?;
        self.client
            .set(BitcoindClient::new(&self.settings).await?)
            .unwrap_or_default();
        Ok(())
    }

    pub fn cookie_path(&self) -> String {
        let dir = if self.network == "mainnet" {
            self.manager.storage_dir.clone()
        } else {
            format!("{}/{}", self.manager.storage_dir, self.network)
        };
        format!("{dir}/.cookie")
    }

    pub fn test_bitcoin(output_dir: &str, settings: &Settings) -> Result<BitcoinManager> {
        let p2p_port = get_available_port().unwrap();
        let rpc_port = get_available_port().unwrap();

        let manager = Manager::new(output_dir, "bitcoind", &settings.node_id)?;
        Ok(BitcoinManager {
            manager,
            p2p_port,
            rpc_port,
            network: settings.bitcoin_network.to_string(),
            settings: settings.clone(),
            client: OnceCell::new(),
        })
    }

    pub async fn generate_blocks(
        &self,
        n_blocks: u64,
        address: &Address,
        delay: bool,
    ) -> Result<()> {
        let client = self.client.get().context("bitcoind not started")?;
        for _ in 0..n_blocks {
            client.generate_to_address(1, address).await?;
            // Sometimes a delay is needed to make the test more realistic which is expected by LDK.
            if delay {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
        client.wait_for_blockchain_synchronisation().await;
        Ok(())
    }
}

pub struct BitcoindCheck(pub Settings);

#[async_trait]
impl Check for BitcoindCheck {
    async fn check(&self) -> bool {
        BitcoindClient::new(&self.0).await.is_ok()
    }
}

#[macro_export]
macro_rules! bitcoin {
    ($settings:expr) => {{
        let mut bitcoind = test_utils::bitcoin_manager::BitcoinManager::test_bitcoin(
            env!("CARGO_TARGET_TMPDIR"),
            &$settings,
        )?;
        $settings.bitcoind_rpc_port = bitcoind.rpc_port;
        $settings.bitcoin_cookie_path = bitcoind.cookie_path();
        bitcoind.settings.bitcoind_rpc_port = $settings.bitcoind_rpc_port;
        bitcoind.settings.bitcoin_cookie_path = $settings.bitcoin_cookie_path.clone();
        bitcoind
            .start(test_utils::bitcoin_manager::BitcoindCheck(
                bitcoind.settings.clone(),
            ))
            .await?;
        bitcoind
    }};
}
