use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use crate::api::SocketAddress;
use crate::database::forward::ForwardStatus;
use crate::ldk::htlc_destination_to_string;
use anyhow::Context;
use api::ChannelFee;
use api::FundChannel;
use api::FundChannelResponse;
use api::SetChannelFee;
use api::SetChannelFeeResponse;
use axum::extract::Path;
use axum::extract::Query;
use axum::{response::IntoResponse, Extension, Json};
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use lightning::events::HTLCDestination;
use lightning::ln::channelmanager::ChannelDetails;
use lightning::ln::features::ChannelTypeFeatures;
use lightning::ln::ChannelId;
use lightning::util::config::MaxDustHTLCExposure;

use crate::api::bad_request;
use crate::ldk::LightningInterface;
use crate::ldk::PeerStatus;
use crate::to_string_empty;

use super::codegen::get_v1_channel_history_response::GetV1ChannelHistoryResponseItem;
use super::codegen::get_v1_channel_list_forwards_response::{
    GetV1ChannelListForwardsResponseItem, GetV1ChannelListForwardsResponseItemStatus,
};
use super::codegen::get_v1_channel_list_peer_channels_response::GetV1ChannelListPeerChannelsResponseState;
use super::codegen::get_v1_channel_list_peer_channels_response::{
    GetV1ChannelListPeerChannelsResponse, GetV1ChannelListPeerChannelsResponseOpener,
};
use super::codegen::get_v1_channel_localremotebal_response::GetV1ChannelLocalremotebalResponse;
use super::internal_server;
use super::ApiError;

pub(crate) async fn list_peer_channels(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, ApiError> {
    let peers = lightning_interface
        .list_peers()
        .await
        .map_err(internal_server)?;

    let channels = lightning_interface.list_channels();
    let mut response = vec![];
    for channel in channels {
        let config = channel
            .config
            .context("expected channel config")
            .map_err(internal_server)?;

        response.push(GetV1ChannelListPeerChannelsResponse {
            alias: lightning_interface
                .alias_of(&channel.counterparty.node_id)
                .unwrap_or_default(),
            channel_id: channel.channel_id.to_hex(),
            dust_limit_msat: match config.max_dust_htlc_exposure {
                MaxDustHTLCExposure::FixedLimitMsat(x) => x as i64,
                MaxDustHTLCExposure::FeeRateMultiplier(x) => x as i64,
            },
            features: channel
                .channel_type
                .map(format_features)
                .unwrap_or_default(),
            fee_base_msat: config.forwarding_fee_base_msat as i64,
            fee_proportional_millionths: config.forwarding_fee_proportional_millionths as i64,
            funding: None,
            funding_txid: channel.funding_txo.map(|x| x.txid.to_string()),
            htlcs: None,
            opener: if channel.is_outbound {
                GetV1ChannelListPeerChannelsResponseOpener::Local
            } else {
                GetV1ChannelListPeerChannelsResponseOpener::Remote
            },
            peer_connected: peers
                .iter()
                .find(|p| p.public_key == channel.counterparty.node_id)
                .map(|p| p.status == PeerStatus::Connected)
                .unwrap_or_default(),
            peer_id: channel.counterparty.node_id.to_string(),
            private: !channel.is_public,
            receivable_msat: channel.inbound_capacity_msat as i64,
            short_channel_id: channel.short_channel_id.map(|x| x.to_string()),
            spendable_msat: channel.outbound_capacity_msat as i64,
            state: if channel.is_usable {
                GetV1ChannelListPeerChannelsResponseState::ChanneldNormal
            } else {
                GetV1ChannelListPeerChannelsResponseState::Openingd
            },
            our_reserve_msat: channel
                .unspendable_punishment_reserve
                .map(|x| (x * 1000) as i64),
            their_reserve_msat: (channel.counterparty.unspendable_punishment_reserve * 1000) as i64,
            to_them_msat: ((channel.channel_value_satoshis * 1000) - channel.balance_msat) as i64,
            minimum_htlc_in_msat: channel.inbound_htlc_minimum_msat.map(|x| x as i64),
            max_total_htlc_in_msat: channel.inbound_htlc_maximum_msat.map(|x| x as i64),
            minimum_htlc_out_msat: channel.next_outbound_htlc_minimum_msat as i64,
            maximum_htlc_out_msat: channel.next_outbound_htlc_limit_msat as i64,
            to_us_msat: channel.balance_msat as i64,
            total_msat: channel.channel_value_satoshis as i64 * 1000,
        })
    }
    Ok(Json(response))
}

pub(crate) async fn open_channel(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Json(fund_channel): Json<FundChannel>,
) -> Result<impl IntoResponse, ApiError> {
    let (public_key, net_address) = match fund_channel.id.split_once('@') {
        Some((public_key, net_address)) => (
            PublicKey::from_str(public_key).map_err(bad_request)?,
            Some(net_address.parse::<SocketAddress>().map_err(bad_request)?),
        ),
        None => (
            PublicKey::from_str(&fund_channel.id).map_err(bad_request)?,
            None,
        ),
    };
    lightning_interface
        .connect_peer(public_key, net_address)
        .await
        .map_err(internal_server)?;

    let value = fund_channel.satoshis.parse::<u64>().map_err(bad_request)?;
    let push_msat = fund_channel
        .push_msat
        .map(|x| x.parse::<u64>())
        .transpose()
        .map_err(bad_request)?;

    let mut user_config = lightning_interface.user_config();
    if let Some(announce) = fund_channel.announce {
        user_config.channel_handshake_config.announced_channel = announce;
    }

    let result = lightning_interface
        .open_channel(
            public_key,
            value,
            push_msat,
            fund_channel.fee_rate,
            Some(user_config),
        )
        .await
        .map_err(internal_server)?;

    let response = FundChannelResponse {
        tx: result.transaction,
        txid: result.txid.to_string(),
        channel_id: result.channel_id.to_hex(),
    };
    Ok(Json(response))
}

pub(crate) async fn set_channel_fee(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Json(channel_fee): Json<ChannelFee>,
) -> Result<impl IntoResponse, ApiError> {
    let mut updated_channels = vec![];

    if channel_fee.id == "all" {
        let mut peer_channels: HashMap<PublicKey, Vec<ChannelDetails>> = HashMap::new();
        for channel in lightning_interface.list_channels() {
            if let Some(channel_ids) = peer_channels.get_mut(&channel.counterparty.node_id) {
                channel_ids.push(channel);
            } else {
                peer_channels.insert(channel.counterparty.node_id, vec![channel]);
            }
        }
        for (node_id, channels) in peer_channels {
            let channel_ids: Vec<ChannelId> = channels.iter().map(|c| c.channel_id).collect();
            let (base, ppm) = lightning_interface
                .set_channel_fee(&node_id, &channel_ids, channel_fee.ppm, channel_fee.base)
                .map_err(internal_server)?;
            for channel in channels {
                updated_channels.push(SetChannelFee {
                    base,
                    ppm,
                    peer_id: node_id.to_string(),
                    channel_id: channel.channel_id.to_hex(),
                    short_channel_id: to_string_empty!(channel.short_channel_id),
                });
            }
        }
    } else if let Some(channel) = lightning_interface.list_channels().iter().find(|c| {
        c.channel_id.to_hex() == channel_fee.id
            || c.short_channel_id.unwrap_or_default().to_string() == channel_fee.id
    }) {
        let (base, ppm) = lightning_interface
            .set_channel_fee(
                &channel.counterparty.node_id,
                &[channel.channel_id],
                channel_fee.ppm,
                channel_fee.base,
            )
            .map_err(internal_server)?;
        updated_channels.push(SetChannelFee {
            base,
            ppm,
            peer_id: channel.counterparty.node_id.to_string(),
            channel_id: channel.channel_id.to_hex(),
            short_channel_id: to_string_empty!(channel.short_channel_id),
        });
    } else {
        return Err(ApiError::NotFound(channel_fee.id));
    }

    Ok(Json(SetChannelFeeResponse(updated_channels)))
}

pub(crate) async fn close_channel(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Path(channel_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    if let Some(channel) = lightning_interface.list_channels().iter().find(|c| {
        c.channel_id.to_hex() == channel_id
            || c.short_channel_id.unwrap_or_default().to_string() == channel_id
    }) {
        lightning_interface
            .close_channel(&channel.channel_id, &channel.counterparty.node_id)
            .await
            .map_err(internal_server)?;
        Ok(Json(()))
    } else {
        Err(ApiError::NotFound(channel_id))
    }
}

pub(crate) async fn local_remote_balance(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, ApiError> {
    let mut local_msat = 0;
    let mut remote_msat = 0;
    let mut pending_msat = 0;
    let mut inactive_msat = 0;
    for channel in lightning_interface.list_channels() {
        if channel.is_usable {
            local_msat += channel.balance_msat;
            remote_msat += (channel.channel_value_satoshis * 1000) - channel.balance_msat;
        } else if channel.is_channel_ready {
            inactive_msat += channel.balance_msat;
        } else {
            pending_msat += channel.balance_msat;
        }
    }
    Ok(Json(GetV1ChannelLocalremotebalResponse {
        inactive_balance: (inactive_msat / 1000) as i64,
        pending_balance: (pending_msat / 1000) as i64,
        local_balance: (local_msat / 1000) as i64,
        remote_balance: (remote_msat / 1000) as i64,
    }))
}

// Paperclip generates an enum but we need a struct to work with axum so have to make query params this way for now.
#[derive(Serialize, Deserialize)]
pub struct ListForwardsQueryParams {
    pub status: Option<GetV1ChannelListForwardsResponseItemStatus>,
}

pub(crate) async fn list_forwards(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Query(params): Query<ListForwardsQueryParams>,
) -> Result<impl IntoResponse, ApiError> {
    let status = match params.status {
        None => None,
        Some(GetV1ChannelListForwardsResponseItemStatus::Settled) => Some(ForwardStatus::Succeeded),
        Some(GetV1ChannelListForwardsResponseItemStatus::Offered) => Some(ForwardStatus::Succeeded),
        _ => Some(ForwardStatus::Failed),
    };
    let mut response = vec![];
    for forward in lightning_interface
        .fetch_forwards(status)
        .await
        .map_err(internal_server)?
    {
        response.push(GetV1ChannelListForwardsResponseItem {
            failcode: match forward.htlc_destination {
                Some(HTLCDestination::NextHopChannel {
                    node_id: _,
                    channel_id: _,
                }) => Some("NextHopChannel".to_string()),
                Some(HTLCDestination::UnknownNextHop {
                    requested_forward_scid: _,
                }) => Some("UnknownNextHop".to_string()),
                Some(HTLCDestination::InvalidForward {
                    requested_forward_scid: _,
                }) => Some("InvalidFormat".to_string()),
                Some(HTLCDestination::FailedPayment { payment_hash: _ }) => {
                    Some("FailedPayment".to_string())
                }
                None => None,
            },
            failreason: forward
                .htlc_destination
                .as_ref()
                .map(htlc_destination_to_string),
            fee_msat: forward.fee.map(|x| x as i64),
            in_channel: forward.inbound_channel_id.to_hex(),
            in_msat: forward.amount.map(|x| x as i64),
            out_channel: forward.outbound_channel_id.map(|x| x.to_hex()),
            out_msat: forward
                .amount
                .and_then(|a| forward.fee.map(|f| (a - f) as i64)),
            payment_hash: match forward.htlc_destination {
                Some(HTLCDestination::FailedPayment { payment_hash }) => {
                    Some(payment_hash.0.to_hex())
                }
                _ => None,
            },
            received_time: forward.timestamp.unix_timestamp(),
            resolved_time: Some(forward.timestamp.unix_timestamp()),
            status: match forward.status {
                ForwardStatus::Succeeded => GetV1ChannelListForwardsResponseItemStatus::Settled,
                ForwardStatus::Failed => GetV1ChannelListForwardsResponseItemStatus::Failed,
            },
        });
    }

    Ok(Json(response))
}

pub(crate) async fn channel_history(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, ApiError> {
    let channel_history = lightning_interface
        .channel_history()
        .await
        .map_err(internal_server)?;

    let mut response = vec![];

    for channel in channel_history {
        response.push(GetV1ChannelHistoryResponseItem {
            close_timestamp: channel
                .close_timestamp
                .context("expected close timestamp")
                .map_err(internal_server)?
                .unix_timestamp(),
            closure_reason: channel
                .closure_reason
                .context("expected closure reason")
                .map_err(internal_server)?
                .to_string(),
            counterparty: channel.counterparty.to_string(),
            funding_txo: format!("{}:{}", channel.funding_txo.txid, channel.funding_txo.index),
            id: channel.id.to_hex(),
            is_outbound: channel.is_outbound,
            is_public: channel.is_public,
            open_timestamp: channel.open_timestamp.unix_timestamp(),
            scid: channel.scid as i64,
            user_channel_id: channel.user_channel_id as i64,
            value: channel.value as i64,
        });
    }

    Ok(Json(response))
}

fn format_features(channel_type: ChannelTypeFeatures) -> Vec<String> {
    channel_type
        .to_string()
        .split(", ")
        .filter_map(|feature| {
            let (k, v) = feature.split_once(": ")?;
            match v {
                "supported" => Some(k.to_string()),
                "required" => Some(k.to_string()),
                _ => None,
            }
        })
        .collect()
}
