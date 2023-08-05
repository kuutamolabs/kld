use crate::api::NetAddress;
use anyhow::anyhow;
use api::{FeeRates, FeeRatesResponse, NetworkChannel, NetworkNode, OnChainFeeEstimates};
use axum::{extract::Path, response::IntoResponse, Extension, Json};
use bitcoin::{hashes::hex::ToHex, secp256k1::PublicKey};
use lightning::routing::gossip::{ChannelInfo, ChannelUpdateInfo, NodeId, NodeInfo};
use std::{str::FromStr, sync::Arc};

use crate::{bitcoind::bitcoind_interface::BitcoindInterface, ldk::LightningInterface};

use super::{bad_request, internal_server, ApiError};

pub(crate) async fn list_network_nodes(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, ApiError> {
    let nodes: Vec<NetworkNode> = lightning_interface
        .nodes()
        .unordered_iter()
        .filter_map(|(node_id, announcement)| to_api_node(node_id, announcement))
        .collect();
    Ok(Json(nodes))
}

pub(crate) async fn get_network_node(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let public_key = PublicKey::from_str(&id).map_err(bad_request)?;
    let node_id = NodeId::from_pubkey(&public_key);
    if let Some(node_info) = lightning_interface.get_node(&node_id) {
        if let Some(node) = to_api_node(&node_id, &node_info) {
            return Ok(Json(vec![node]));
        }
    }
    Err(ApiError::NotFound(id))
}

pub(crate) async fn get_network_channel(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let short_channel_id = u64::from_str(&id).map_err(bad_request)?;
    if let Some(channel_info) = lightning_interface.get_channel(short_channel_id) {
        return Ok(Json(to_api_channel(&short_channel_id, &channel_info)));
    }
    Err(ApiError::NotFound(id))
}

pub(crate) async fn list_network_channels(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, ApiError> {
    let mut channels = vec![];
    for (short_channel_id, channel_info) in lightning_interface.channels().unordered_iter() {
        channels.append(&mut to_api_channel(short_channel_id, channel_info))
    }
    Ok(Json(channels))
}

const CHANNEL_OPEN_VB: u32 = 152;
const MUTUAL_CLOSE_VB: u32 = 130;
const UNILATERAL_CLOSE_VB: u32 = 150;

pub(crate) async fn fee_rates(
    Extension(bitcoind_interface): Extension<Arc<dyn BitcoindInterface + Send + Sync>>,
    Path(style): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let (urgent, normal, slow) = bitcoind_interface.fee_rates_kw();
    let mempool_info = bitcoind_interface
        .get_mempool_info()
        .await
        .map_err(internal_server)?;

    let onchain_fee_estimates = OnChainFeeEstimates {
        opening_channel_satoshis: ((normal as f32 / 1000.0) * CHANNEL_OPEN_VB as f32 * 4.0) as u32,
        mutual_close_satoshis: ((normal as f32 / 1000.0) * MUTUAL_CLOSE_VB as f32 * 4.0) as u32,
        unilateral_close_satoshis: ((normal as f32 / 1000.0) * UNILATERAL_CLOSE_VB as f32 * 4.0)
            as u32,
    };
    let response = match style.as_str() {
        "perkb" => {
            let fee_rates = FeeRates {
                urgent: urgent * 4,
                normal: normal * 4,
                slow: slow * 4,
                min_acceptable: (mempool_info.mempool_min_fee * 100000000.0) as u32,
                max_acceptable: urgent * 4,
            };
            FeeRatesResponse {
                perkb: Some(fee_rates),
                perkw: None,
                onchain_fee_estimates,
            }
        }
        "perkw" => {
            let fee_rates = FeeRates {
                urgent,
                normal,
                slow,
                min_acceptable: (mempool_info.mempool_min_fee * 25000000.0) as u32,
                max_acceptable: urgent,
            };
            FeeRatesResponse {
                perkb: None,
                perkw: Some(fee_rates),
                onchain_fee_estimates,
            }
        }
        _ => return Err(bad_request(anyhow!("unknown fee style {}", style))),
    };
    Ok(Json(response))
}

fn to_api_channel(short_channel_id: &u64, channel_info: &ChannelInfo) -> Vec<NetworkChannel> {
    let mut channels = vec![];

    let make_channel =
        |node_one: NodeId, node_two: NodeId, update_info: &ChannelUpdateInfo| NetworkChannel {
            source: node_one.as_slice().to_hex(),
            destination: node_two.as_slice().to_hex(),
            short_channel_id: *short_channel_id,
            public: true,
            satoshis: channel_info.capacity_sats.unwrap_or_default(),
            amount_msat: channel_info
                .capacity_sats
                .map(|s| s * 1000)
                .unwrap_or_default(),
            channel_flags: update_info
                .last_update_message
                .as_ref()
                .map_or(0, |m| m.contents.flags),
            active: update_info.enabled,
            last_update: update_info.last_update,
            base_fee_millisatoshi: update_info.fees.base_msat,
            fee_per_millionth: update_info.fees.proportional_millionths,
            delay: update_info.cltv_expiry_delta,
            htlc_minimum_msat: update_info.htlc_minimum_msat,
            htlc_maximum_msat: update_info.htlc_maximum_msat,
        };
    if let Some(update_info) = &channel_info.one_to_two {
        channels.push(make_channel(
            channel_info.node_one,
            channel_info.node_two,
            update_info,
        ));
    }
    if let Some(update_info) = &channel_info.two_to_one {
        channels.push(make_channel(
            channel_info.node_two,
            channel_info.node_one,
            update_info,
        ));
    }
    channels
}

fn to_api_node(node_id: &NodeId, node_info: &NodeInfo) -> Option<NetworkNode> {
    node_info.announcement_info.as_ref().map(|n| NetworkNode {
        node_id: node_id.as_slice().to_hex(),
        alias: n.alias.to_string(),
        color: n.rgb.to_hex(),
        last_timestamp: n.last_update,
        features: n.features.to_string(),
        addresses: n
            .addresses()
            .iter()
            .map(|a| NetAddress(a.clone()).to_string())
            .collect(),
    })
}
