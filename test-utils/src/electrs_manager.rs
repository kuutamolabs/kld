use anyhow::Result;
use async_trait::async_trait;
use kld::settings::Settings;
use tempfile::TempDir;

use crate::{
    manager::{Check, Manager},
    ports::get_available_port,
    BitcoinManager,
};

pub struct ElectrsManager<'a> {
    manager: Manager<'a>,
    pub rpc_address: String,
    pub monitoring_addr: String,
    bitcoin_rpc_addr: String,
    bitcoin_p2p_addr: String,
    bitcoin_cookie_path: String,
    bitcoin_network: String,
}

impl<'a> ElectrsManager<'a> {
    pub async fn new(
        output_dir: &'a TempDir,
        bitcoin_manager: &BitcoinManager<'a>,
        settings: &mut Settings,
    ) -> Result<ElectrsManager<'a>> {
        let monitoring_port = get_available_port()?;
        let rpc_port = get_available_port()?;

        let manager = Manager::new(output_dir, "electrs", &settings.node_id)?;
        let mut electrs = ElectrsManager {
            manager,
            rpc_address: format!("127.0.0.1:{rpc_port}"),
            monitoring_addr: format!("127.0.0.1:{monitoring_port}"),
            bitcoin_rpc_addr: format!("127.0.0.1:{}", settings.bitcoind_rpc_port),
            bitcoin_p2p_addr: format!("127.0.0.1:{}", bitcoin_manager.p2p_port),
            bitcoin_cookie_path: settings.bitcoin_cookie_path.clone(),
            bitcoin_network: settings.bitcoin_network.to_string(),
        };

        settings.electrs_url = electrs.rpc_address.clone();
        electrs
            .start(ElectrsCheck(electrs.monitoring_addr.clone()))
            .await?;
        Ok(electrs)
    }

    pub async fn start(&mut self, check: impl Check) -> Result<()> {
        let args = &[
            "--skip-default-conf-files",
            "--log-filters=DEBUG",
            &format!("--network={}", &self.bitcoin_network),
            &format!("--db-dir={}", &self.manager.storage_dir.as_path().display()),
            &format!("--cookie-file={}", &self.bitcoin_cookie_path),
            &format!("--electrum-rpc-addr={}", &self.rpc_address),
            &format!("--daemon-rpc-addr={}", &self.bitcoin_rpc_addr),
            &format!("--daemon-p2p-addr={}", &self.bitcoin_p2p_addr),
            &format!("--monitoring-addr={}", &self.monitoring_addr),
        ];
        self.manager.start("electrs", args, check).await
    }
}

pub struct ElectrsCheck(pub String);

#[async_trait]
impl Check for ElectrsCheck {
    async fn check(&self) -> bool {
        if let Ok(res) = reqwest::get(format!("http://{}", self.0)).await {
            if res.status().is_success() {
                return true;
            }
        }
        return false;
    }
}
