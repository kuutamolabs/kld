use std::fs::File;

use async_trait::async_trait;

use crate::{
    manager::{Manager, Starts},
    ports::get_available_port,
};

const NETWORK: &str = "regtest";

pub struct BitcoinManager {
    manager: Manager,
    rpc_auth: String,
    pub p2p_port: u16,
    pub rpc_port: u16,
    pub network: String,
}

impl BitcoinManager {
    pub async fn start(&mut self) {
        let args = &[
            "-server",
            "-noconnect",
            &format!("-chain={}", NETWORK),
            &format!("-datadir={}", &self.manager.storage_dir),
            &format!("-port={}", &self.p2p_port.to_string()),
            &format!("-rpcport={}", &self.rpc_port.to_string()),
            &format!("-rpcauth={}", &self.rpc_auth),
        ];
        self.manager.start("bitcoind", args).await;
        // Getting occasional bad file descriptor in tests. Maybe this helps.
        File::open(self.cookie_path()).unwrap().sync_all().unwrap();
    }

    pub fn cookie_path(&self) -> String {
        format!("{}/.cookie", self.data_dir())
    }

    pub fn test_bitcoin(output_dir: &str, node_index: u16) -> BitcoinManager {
        let p2p_port = get_available_port().unwrap();
        let rpc_port = get_available_port().unwrap();
        // user:password just used by TEOS at the moment.
        let rpc_auth = "user:bcae5b9986aa90ef40565c2b5d5e685c$8d81897118da8bc7489619853f68e1fc161b3e4cb904071ea123965136468b81".to_string();

        let manager = Manager::new(
            Box::new(BitcoinApi(format!("http://127.0.0.1:{}", rpc_port))),
            output_dir,
            "bitcoind",
            node_index,
        );

        BitcoinManager {
            manager,
            p2p_port,
            rpc_port,
            rpc_auth,
            network: NETWORK.to_string(),
        }
    }

    fn data_dir(&self) -> String {
        if NETWORK == "mainnet" {
            self.manager.storage_dir.clone()
        } else {
            format!("{}/{}", self.manager.storage_dir, NETWORK)
        }
    }
}

pub struct BitcoinApi(String);

#[async_trait]
impl Starts for BitcoinApi {
    async fn has_started(&self) -> bool {
        reqwest::get(&self.0).await.is_ok()
    }
}

#[macro_export]
macro_rules! bitcoin {
    () => {
        test_utils::bitcoin_manager::BitcoinManager::test_bitcoin(env!("CARGO_TARGET_TMPDIR"), 0)
    };
    ($n:literal) => {
        test_utils::bitcoin_manager::BitcoinManager::test_bitcoin(env!("CARGO_TARGET_TMPDIR"), $n)
    };
}
