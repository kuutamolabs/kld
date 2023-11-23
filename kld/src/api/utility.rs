use anyhow::anyhow;
use api::{Chain, GetInfo};
use api::{SignRequest, SignResponse, API_VERSION};
use axum::Json;
use axum::{response::IntoResponse, Extension};
use bitcoin::Network;
use lightning::routing::gossip::NodeId;
use std::str::FromStr;
use std::sync::Arc;

use crate::bitcoind::bitcoind_interface::BitcoindInterface;
use crate::ldk::LightningInterface;
use crate::VERSION;

use super::codegen::get_v1_estimate_channel_liquidity_body::GetV1EstimateChannelLiquidityBody;
use super::codegen::get_v1_estimate_channel_liquidity_response::GetV1EstimateChannelLiquidityResponse;
use super::codegen::get_v1_get_fees_response::GetV1GetFeesResponse;
use super::{bad_request, internal_server, ApiError};

pub(crate) async fn get_info(
    Extension(bitcoind_interface): Extension<Arc<dyn BitcoindInterface + Send + Sync>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, ApiError> {
    let synced_to_chain = lightning_interface
        .synced()
        .await
        .map_err(internal_server)?;
    let fees_collected_msat = lightning_interface
        .fetch_total_forwards()
        .await
        .map_err(internal_server)?
        .fee;
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
            .into_iter()
            .map(|a| a.to_string())
            .collect(),
        fees_collected_msat,
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

pub(crate) async fn estimate_channel_liquidity_range(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Json(body): Json<GetV1EstimateChannelLiquidityBody>,
) -> Result<impl IntoResponse, ApiError> {
    let node_id = NodeId::from_str(&body.target).map_err(bad_request)?;
    match lightning_interface
        .estimated_channel_liquidity_range(body.scid as u64, &node_id)
        .await
        .map_err(internal_server)?
    {
        Some((minimum, maximum)) => Ok(Json(GetV1EstimateChannelLiquidityResponse {
            minimum: minimum as i64,
            maximum: maximum as i64,
        })),
        None => Err(ApiError::NotFound(body.scid.to_string())),
    }
}

pub(crate) async fn get_fees(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, ApiError> {
    let total_forwards = lightning_interface
        .fetch_total_forwards()
        .await
        .map_err(internal_server)?;
    let response = GetV1GetFeesResponse {
        fee_collected: total_forwards.fee as i64,
    };
    Ok(Json(response))
}

pub(crate) async fn score(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, ApiError> {
    let score = lightning_interface
        .scorer()
        .await
        .map_err(internal_server)?;
    Ok(score)
}
