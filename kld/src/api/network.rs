use api::{Address, NetworkChannel, NetworkNode};
use axum::{extract::Path, response::IntoResponse, Extension, Json};
use bitcoin::secp256k1::PublicKey;
use hex::ToHex;
use lightning::{
    ln::msgs::NetAddress,
    routing::gossip::{ChannelInfo, DirectedChannelInfo, NodeId, NodeInfo},
};
use std::{
    net::{Ipv4Addr, Ipv6Addr},
    str::FromStr,
    sync::Arc,
};

use crate::ldk::LightningInterface;

use super::{bad_request, unauthorized, ApiError, KldMacaroon, MacaroonAuth};

pub(crate) async fn list_network_nodes(
    macaroon: KldMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, ApiError> {
    macaroon_auth
        .verify_readonly_macaroon(&macaroon.0)
        .map_err(unauthorized)?;
    let nodes: Vec<NetworkNode> = lightning_interface
        .nodes()
        .unordered_iter()
        .filter_map(|(node_id, announcement)| to_api_node(node_id, announcement))
        .collect();
    Ok(Json(nodes))
}

pub(crate) async fn get_network_node(
    macaroon: KldMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    macaroon_auth
        .verify_readonly_macaroon(&macaroon.0)
        .map_err(unauthorized)?;
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
    macaroon: KldMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    macaroon_auth
        .verify_readonly_macaroon(&macaroon.0)
        .map_err(unauthorized)?;
    let short_channel_id = u64::from_str(&id).map_err(bad_request)?;
    if let Some(channel_info) = lightning_interface.get_channel(short_channel_id) {
        if let Some((directed_info, _)) = channel_info.as_directed_to(&channel_info.node_one) {
            if let Some(api_channel) =
                to_api_channel(&short_channel_id, &channel_info, &directed_info)
            {
                return Ok(Json(vec![api_channel]));
            }
        }
    }
    Err(ApiError::NotFound(id))
}

pub(crate) async fn list_network_channels(
    macaroon: KldMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, ApiError> {
    macaroon_auth
        .verify_readonly_macaroon(&macaroon.0)
        .map_err(unauthorized)?;
    let mut channels = vec![];
    for (short_channel_id, channel_info) in lightning_interface.channels().unordered_iter() {
        if let Some((directed_info, _)) = channel_info.as_directed_to(&channel_info.node_one) {
            if let Some(api_channel) =
                to_api_channel(short_channel_id, channel_info, &directed_info)
            {
                channels.push(api_channel);
            }
        }
        if let Some((directed_info, _)) = channel_info.as_directed_to(&channel_info.node_two) {
            if let Some(api_channel) =
                to_api_channel(short_channel_id, channel_info, &directed_info)
            {
                channels.push(api_channel);
            }
        }
    }
    Ok(Json(channels))
}

fn to_api_channel(
    short_channel_id: &u64,
    channel_info: &ChannelInfo,
    directed_info: &DirectedChannelInfo,
) -> Option<NetworkChannel> {
    directed_info
        .channel()
        .one_to_two
        .as_ref()
        .map(|channel_update| NetworkChannel {
            source: directed_info.channel().node_one.as_slice().encode_hex(),
            destination: directed_info.channel().node_two.as_slice().encode_hex(),
            short_channel_id: *short_channel_id,
            public: true,
            satoshis: channel_info.capacity_sats.unwrap_or_default(),
            amount_msat: channel_info
                .capacity_sats
                .map(|s| s * 1000)
                .unwrap_or_default(),
            message_flags: 0,
            channel_flags: 0,
            description: String::new(),
            active: channel_update.enabled,
            last_update: channel_update.last_update,
            base_fee_millisatoshi: channel_update.fees.base_msat,
            fee_per_millionth: channel_update.fees.proportional_millionths,
            delay: channel_update.cltv_expiry_delta,
            htlc_minimum_msat: channel_update.htlc_minimum_msat,
            htlc_maximum_msat: channel_update.htlc_maximum_msat,
        })
}

fn to_api_node(node_id: &NodeId, node_info: &NodeInfo) -> Option<NetworkNode> {
    node_info.announcement_info.as_ref().map(|n| NetworkNode {
        node_id: node_id.as_slice().encode_hex(),
        alias: n.alias.to_string(),
        color: n.rgb.encode_hex(),
        last_timestamp: n.last_update,
        features: n.features.to_string(),
        addresses: n.addresses.iter().map(to_api_address).collect(),
    })
}

pub(crate) fn to_api_address(net_address: &NetAddress) -> Address {
    match net_address {
        NetAddress::IPv4 { addr, port } => Address {
            address_type: "ipv4".to_string(),
            address: Ipv4Addr::from(*addr).to_string(),
            port: *port,
        },
        NetAddress::IPv6 { addr, port } => Address {
            address_type: "ipv6".to_string(),
            address: Ipv6Addr::from(*addr).to_string(),
            port: *port,
        },
        NetAddress::OnionV2(pubkey) => Address {
            address_type: "onionv2".to_string(),
            address: pubkey.encode_hex(),
            port: 0,
        },
        NetAddress::OnionV3 {
            ed25519_pubkey,
            checksum: _,
            version: _,
            port,
        } => Address {
            address_type: "onionv3".to_string(),
            address: ed25519_pubkey.encode_hex(),
            port: *port,
        },
        NetAddress::Hostname { hostname, port } => Address {
            address_type: "hostname".to_string(),
            address: hostname.to_string(),
            port: *port,
        },
    }
}
