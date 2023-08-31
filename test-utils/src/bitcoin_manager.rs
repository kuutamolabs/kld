use std::{sync::OnceLock, time::Duration};

use anyhow::{Context, Result};
use async_trait::async_trait;
use bitcoin::Address;
use kld::bitcoind::BitcoindClient;
use kld::settings::Settings;
use tempfile::TempDir;

use crate::{
    manager::{Check, Manager},
    ports::get_available_port,
};

pub struct BitcoinManager<'a> {
    manager: Manager<'a>,
    pub p2p_port: u16,
    pub rpc_port: u16,
    pub network: String,
    pub settings: Settings,
    pub client: OnceLock<BitcoindClient>,
}

impl<'a> BitcoinManager<'a> {
    pub async fn new(
        output_dir: &'a TempDir,
        settings: &mut Settings,
    ) -> Result<BitcoinManager<'a>> {
        let p2p_port = get_available_port()?;
        let rpc_port = get_available_port()?;

        let manager = Manager::new(output_dir, "bitcoind", &settings.node_id)?;
        let mut bitcoind = BitcoinManager {
            manager,
            p2p_port,
            rpc_port,
            network: settings.bitcoin_network.to_string(),
            settings: settings.clone(),
            client: OnceLock::new(),
        };
        settings.bitcoind_rpc_port = bitcoind.rpc_port;
        settings.bitcoin_cookie_path = bitcoind.cookie_path();
        bitcoind.settings.bitcoind_rpc_port = settings.bitcoind_rpc_port;
        bitcoind.settings.bitcoin_cookie_path = settings.bitcoin_cookie_path.clone();
        bitcoind
            .start(BitcoindCheck(bitcoind.settings.clone()))
            .await?;
        Ok(bitcoind)
    }

    pub async fn start(&mut self, check: impl Check) -> Result<()> {
        let args = &[
            "-server",
            "-noconnect",
            "-rpcthreads=16",
            "-listen",
            &format!("-chain={}", &self.network),
            &format!("-datadir={}", &self.manager.storage_dir.as_path().display()),
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
            self.manager.storage_dir.join(&self.network)
        };
        dir.join(".cookie")
            .into_os_string()
            .into_string()
            .expect("should not use non UTF-8 char in path")
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
