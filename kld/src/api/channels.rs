use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use api::Channel;
use api::ChannelFee;
use api::FundChannel;
use api::FundChannelResponse;
use api::SetChannelFee;
use api::SetChannelFeeResponse;
use axum::extract::Path;
use axum::{response::IntoResponse, Extension, Json};
use bitcoin::secp256k1::PublicKey;
use hex::ToHex;
use lightning::ln::channelmanager::ChannelDetails;

use crate::api::bad_request;
use crate::ldk::net_utils::PeerAddress;
use crate::ldk::LightningInterface;
use crate::ldk::PeerStatus;
use crate::to_string_empty;

use super::internal_server;
use super::unauthorized;
use super::ApiError;
use super::KldMacaroon;
use super::MacaroonAuth;

pub(crate) async fn list_channels(
    macaroon: KldMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, ApiError> {
    macaroon_auth
        .verify_readonly_macaroon(&macaroon.0)
        .map_err(unauthorized)?;

    let peers = lightning_interface
        .list_peers()
        .await
        .map_err(internal_server)?;

    let channels: Vec<Channel> = lightning_interface
        .list_channels()
        .iter()
        .map(|c| Channel {
            id: c.counterparty.node_id.to_string(),
            connected: peers
                .iter()
                .find(|p| p.public_key == c.counterparty.node_id)
                .map(|p| p.status == PeerStatus::Connected)
                .unwrap_or_default()
                .to_string(),
            state: (if c.is_usable {
                "usable"
            } else if c.is_channel_ready {
                "ready"
            } else {
                "pending"
            })
            .to_string(),
            short_channel_id: to_string_empty!(c.short_channel_id),
            channel_id: c.channel_id.encode_hex(),
            funding_txid: to_string_empty!(c.funding_txo.map(|x| x.txid)),
            private: (!c.is_public).to_string(),
            msatoshi_to_us: c.outbound_capacity_msat.to_string(),
            msatoshi_total: c.channel_value_satoshis.to_string(),
            msatoshi_to_them: c.inbound_capacity_msat.to_string(),
            their_channel_reserve_satoshis: c
                .counterparty
                .unspendable_punishment_reserve
                .to_string(),
            our_channel_reserve_satoshis: to_string_empty!(c.unspendable_punishment_reserve),
            spendable_msatoshi: c.outbound_capacity_msat.to_string(),
            direction: u8::from(c.is_outbound),
            alias: lightning_interface
                .alias_of(&c.counterparty.node_id)
                .unwrap_or_default(),
        })
        .collect();
    Ok(Json(channels))
}

pub(crate) async fn open_channel(
    macaroon: KldMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Json(fund_channel): Json<FundChannel>,
) -> Result<impl IntoResponse, ApiError> {
    macaroon_auth
        .verify_admin_macaroon(&macaroon.0)
        .map_err(unauthorized)?;

    let (public_key, net_address) = match fund_channel.id.split_once('@') {
        Some((public_key, net_address)) => (
            PublicKey::from_str(public_key).map_err(bad_request)?,
            Some(net_address.parse::<PeerAddress>().map_err(bad_request)?),
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
        channel_id: result.channel_id.encode_hex(),
    };
    Ok(Json(response))
}

pub(crate) async fn set_channel_fee(
    macaroon: KldMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Json(channel_fee): Json<ChannelFee>,
) -> Result<impl IntoResponse, ApiError> {
    macaroon_auth
        .verify_admin_macaroon(&macaroon.0)
        .map_err(unauthorized)?;

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
            let channel_ids: Vec<[u8; 32]> = channels.iter().map(|c| c.channel_id).collect();
            let (base, ppm) = lightning_interface
                .set_channel_fee(&node_id, &channel_ids, channel_fee.ppm, channel_fee.base)
                .map_err(internal_server)?;
            for channel in channels {
                updated_channels.push(SetChannelFee {
                    base,
                    ppm,
                    peer_id: node_id.to_string(),
                    channel_id: channel.channel_id.encode_hex(),
                    short_channel_id: to_string_empty!(channel.short_channel_id),
                });
            }
        }
    } else if let Some(channel) = lightning_interface.list_channels().iter().find(|c| {
        c.channel_id.encode_hex::<String>() == channel_fee.id
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
            channel_id: channel.channel_id.encode_hex(),
            short_channel_id: to_string_empty!(channel.short_channel_id),
        });
    } else {
        return Err(ApiError::NotFound(channel_fee.id));
    }

    Ok(Json(SetChannelFeeResponse(updated_channels)))
}

pub(crate) async fn close_channel(
    macaroon: KldMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Path(channel_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    macaroon_auth
        .verify_admin_macaroon(&macaroon.0)
        .map_err(unauthorized)?;

    if let Some(channel) = lightning_interface.list_channels().iter().find(|c| {
        c.channel_id.encode_hex::<String>() == channel_id
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
