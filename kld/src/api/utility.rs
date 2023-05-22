use anyhow::anyhow;
use api::{Address, SignRequest, SignResponse, API_VERSION};
use api::{Chain, GetInfo};
use axum::Json;
use axum::{response::IntoResponse, Extension};
use bitcoin::Network;
use std::sync::Arc;

use crate::bitcoind::bitcoind_interface::BitcoindInterface;
use crate::ldk::LightningInterface;
use crate::VERSION;

use super::{bad_request, internal_server, ApiError};

pub(crate) async fn get_info(
    Extension(bitcoind_interface): Extension<Arc<dyn BitcoindInterface + Send + Sync>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, ApiError> {
    let synced_to_chain = lightning_interface
        .synced()
        .await
        .map_err(internal_server)?;
    let info = GetInfo {
        id: lightning_interface.identity_pubkey().to_string(),
        alias: lightning_interface.alias(),
        num_pending_channels: lightning_interface.num_pending_channels(),
        num_active_channels: lightning_interface.num_active_channels(),
        num_inactive_channels: lightning_interface.num_inactive_channels(),
        num_peers: lightning_interface.num_peers(),
        block_height: bitcoind_interface
            .block_height()
            .await
            .map_err(internal_server)?,
        synced_to_chain,
        testnet: lightning_interface.network() != Network::Bitcoin,
        chains: vec![Chain {
            chain: "bitcoin".to_string(),
            network: lightning_interface.network().to_string(),
        }],
        version: VERSION.to_string(),
        api_version: API_VERSION.to_string(),
        color: "".to_string(),
        network: lightning_interface.network().to_string(),
        address: lightning_interface
            .public_addresses()
            .iter()
            .filter_map(|a| a.split_once(':'))
            .map(|a| Address {
                address_type: "ipv4".to_string(),
                address: a.0.to_string(),
                port: a.1.parse().unwrap_or_default(),
            })
            .collect(),
    };
    Ok(Json(info))
}

const MESSAGE_MAX_LENGTH: u16 = 65535;

pub(crate) async fn sign(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Json(body): Json<SignRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if body.message.len() > MESSAGE_MAX_LENGTH as usize {
        return Err(bad_request(anyhow!(
            "Max message length is {MESSAGE_MAX_LENGTH}"
        )));
    }

    let signature = lightning_interface
        .sign(body.message.as_bytes())
        .map_err(internal_server)?;
    Ok(Json(SignResponse { signature }))
}
