use anyhow::Result;
use std::{fs::File, io::Read};

use async_trait::async_trait;
use base64::{engine::general_purpose, Engine};
use lightning_block_sync::{http::HttpEndpoint, rpc::RpcClient, BlockSource};

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
    pub async fn start(&mut self) -> Result<()> {
        let args = &[
            "-server",
            "-noconnect",
            &format!("-chain={}", NETWORK),
            &format!("-datadir={}", &self.manager.storage_dir),
            &format!("-port={}", &self.p2p_port.to_string()),
            &format!("-rpcport={}", &self.rpc_port.to_string()),
            &format!("-rpcauth={}", &self.rpc_auth),
        ];
        self.manager.start("bitcoind", args).await
    }

    pub fn cookie_path(&self) -> String {
        cookie_path(&self.manager)
    }

    pub fn test_bitcoin(output_dir: &str, node_index: u16) -> BitcoinManager {
        let p2p_port = get_available_port().unwrap();
        let rpc_port = get_available_port().unwrap();
        // user:password just used by TEOS at the moment.
        let rpc_auth = "user:bcae5b9986aa90ef40565c2b5d5e685c$8d81897118da8bc7489619853f68e1fc161b3e4cb904071ea123965136468b81".to_string();

        let manager = Manager::new(
            Box::new(BitcoinApi {
                host: "127.0.0.1".to_string(),
                rpc_port,
            }),
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
}

fn cookie_path(manager: &Manager) -> String {
    let dir = if NETWORK == "mainnet" {
        manager.storage_dir.clone()
    } else {
        format!("{}/{}", manager.storage_dir, NETWORK)
    };
    format!("{}/.cookie", dir)
}

pub struct BitcoinApi {
    host: String,
    rpc_port: u16,
}

#[async_trait]
impl Starts for BitcoinApi {
    async fn has_started(&self, manager: &Manager) -> bool {
        if let Ok(mut file) = File::open(cookie_path(manager)) {
            let mut cookie = String::new();
            file.read_to_string(&mut cookie).unwrap();
            let credentials = general_purpose::STANDARD.encode(cookie.as_bytes());
            let http_endpoint = HttpEndpoint::for_host(self.host.clone()).with_port(self.rpc_port);
            let client = RpcClient::new(&credentials, http_endpoint).unwrap();
            client.get_best_block().await.is_ok()
        } else {
            false
        }
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
