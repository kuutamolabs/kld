use std::sync::Arc;

use axum::{http::StatusCode, Extension, Json};
use bitcoin::{secp256k1::PublicKey, Network};
use serde::{Deserialize, Serialize};

use crate::LightningInterface;

#[derive(Serialize, Deserialize)]
pub struct GetInfo {
    pub identity_pubkey: PublicKey,
    pub alias: String,
    pub num_pending_channels: usize,
    pub num_active_channels: usize,
    pub num_inactive_channels: usize,
    pub num_peers: usize,
    pub block_height: usize,
    pub synced_to_chain: bool,
    pub testnet: bool,
    pub chains: Vec<Chain>,
    pub version: String,
}

#[derive(Serialize, Deserialize)]
pub struct Chain {
    pub chain: String,
    pub network: Network,
}

pub(crate) async fn get_info(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> (StatusCode, Json<GetInfo>) {
    let info = GetInfo {
        identity_pubkey: lightning_interface.identity_pubkey(),
        alias: lightning_interface.alias(),
        num_pending_channels: lightning_interface.num_pending_channels(),
        num_active_channels: lightning_interface.num_active_channels(),
        num_inactive_channels: lightning_interface.num_inactive_channels(),
        num_peers: lightning_interface.num_peers(),
        block_height: lightning_interface.block_height(),
        synced_to_chain: true,
        testnet: lightning_interface.network() != Network::Bitcoin,
        chains: vec![Chain {
            chain: "bitcoin".to_string(),
            network: lightning_interface.network(),
        }],
        version: lightning_interface.version(),
    };

    (StatusCode::FOUND, Json(info))
}
