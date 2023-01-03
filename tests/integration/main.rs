use anyhow::Result;
use bdk::Balance;
use bitcoin::{secp256k1::PublicKey, Network};
use lightning_knd::api::{LightningInterface, WalletInterface};
use test_utils::random_public_key;
use tokio::signal::unix::SignalKind;

pub mod api;
pub mod prometheus;

pub struct MockLightning {
    num_peers: usize,
    num_nodes: usize,
    num_channels: usize,
    wallet_balance: u64,
}

impl Default for MockLightning {
    fn default() -> Self {
        Self {
            num_peers: 5,
            num_nodes: 6,
            num_channels: 7,
            wallet_balance: 8,
        }
    }
}

impl LightningInterface for MockLightning {
    fn alias(&self) -> String {
        "test".to_string()
    }
    fn identity_pubkey(&self) -> PublicKey {
        random_public_key()
    }

    fn graph_num_nodes(&self) -> usize {
        self.num_nodes
    }

    fn graph_num_channels(&self) -> usize {
        self.num_channels
    }

    fn block_height(&self) -> usize {
        50000
    }

    fn network(&self) -> bitcoin::Network {
        Network::Bitcoin
    }
    fn num_active_channels(&self) -> usize {
        0
    }

    fn num_inactive_channels(&self) -> usize {
        0
    }

    fn num_pending_channels(&self) -> usize {
        0
    }
    fn num_peers(&self) -> usize {
        self.num_peers
    }

    fn wallet_balance(&self) -> u64 {
        self.wallet_balance
    }

    fn version(&self) -> String {
        "v0.1".to_string()
    }
}

pub struct MockWallet {
    balance: Balance,
}

impl WalletInterface for MockWallet {
    fn balance(&self) -> Result<bdk::Balance> {
        Ok(self.balance.clone())
    }
}

impl Default for MockWallet {
    fn default() -> Self {
        Self {
            balance: Balance {
                immature: 1,
                trusted_pending: 2,
                untrusted_pending: 3,
                confirmed: 4,
            },
        }
    }
}

pub async fn quit_signal() {
    let _ = tokio::signal::unix::signal(SignalKind::quit())
        .unwrap()
        .recv()
        .await;
}
