use async_trait::async_trait;

use crate::{
    bitcoin_manager::BitcoinManager,
    manager::{Manager, Starts},
    ports::get_available_port,
};
use anyhow::Result;

pub struct TeosManager {
    manager: Manager,
    pub port: u16,
    bitcoin_port: u16,
    bitcoin_network: String,
}

impl TeosManager {
    pub async fn start(&mut self) -> Result<()> {
        let args = &[
            "--apibind=127.0.0.1",
            &format!("--apiport={}", self.port),
            &format!("--btcrpcport={}", self.bitcoin_port),
            &format!("--btcnetwork={}", self.bitcoin_network),
            "--btcrpcuser=user",
            "--btcrpcpassword=password",
            &format!("--datadir={}", self.manager.storage_dir),
        ];
        self.manager.start("teosd", args).await
    }

    pub fn test_teos(output_dir: &str, bitcoin: &BitcoinManager) -> TeosManager {
        let port = get_available_port().unwrap();
        let http_address = format!("http://127.0.0.1:{}/get_subscription_info", port);

        let manager = Manager::new(Box::new(TeosApi(http_address)), output_dir, "teosd", 0);
        TeosManager {
            manager,
            port,
            bitcoin_port: bitcoin.rpc_port,
            bitcoin_network: bitcoin.network.clone(),
        }
    }

    pub fn kill(&mut self) {
        self.manager.kill()
    }
}

pub struct TeosApi(String);

#[async_trait]
impl Starts for TeosApi {
    async fn has_started(&self) -> bool {
        match reqwest::get(self.0.clone()).await {
            Ok(_) => true,
            Err(e) => e.is_status(),
        }
    }
}

#[macro_export]
macro_rules! teos {
    ($bitcoin: expr) => {
        test_utils::teos_manager::TeosManager::test_teos(env!("CARGO_TARGET_TMPDIR"), $bitcoin)
    };
}
