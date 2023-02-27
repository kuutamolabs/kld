use std::collections::HashMap;
use std::sync::Arc;

use api::Channel;
use api::ChannelFee;
use api::FundChannel;
use api::FundChannelResponse;
use api::SetChannelFee;
use api::SetChannelFeeResponse;
use axum::extract::Path;
use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use bitcoin::secp256k1::PublicKey;
use hex::ToHex;
use lightning::ln::channelmanager::ChannelDetails;
use log::{info, warn};

use crate::handle_bad_request;
use crate::handle_err;
use crate::handle_unauthorized;
use crate::to_string_empty;

use super::KndMacaroon;
use super::LightningInterface;
use super::MacaroonAuth;

pub(crate) async fn list_channels(
    macaroon: KndMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, StatusCode> {
    handle_unauthorized!(macaroon_auth.verify_readonly_macaroon(&macaroon.0));

    let channels: Vec<Channel> = lightning_interface
        .list_channels()
        .iter()
        .map(|c| Channel {
            id: c.counterparty.node_id.to_string(),
            connected: c.is_usable.to_string(),
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
            msatoshi_to_us: "".to_string(),
            msatoshi_total: c.channel_value_satoshis.to_string(),
            msatoshi_to_them: "".to_string(),
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
    macaroon: KndMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Json(fund_channel): Json<FundChannel>,
) -> Result<impl IntoResponse, StatusCode> {
    handle_unauthorized!(macaroon_auth.verify_admin_macaroon(&macaroon.0));

    let pub_key_bytes = handle_bad_request!(hex::decode(fund_channel.id));
    let public_key = handle_bad_request!(PublicKey::from_slice(&pub_key_bytes));
    let value = handle_bad_request!(fund_channel.satoshis.parse());
    let push_msat =
        handle_bad_request!(fund_channel.push_msat.map(|x| x.parse::<u64>()).transpose());

    let result = handle_err!(
        lightning_interface
            .open_channel(public_key, value, push_msat, None)
            .await
    );
    let transaction = handle_err!(serde_json::to_string(&result.transaction));
    let response = FundChannelResponse {
        tx: transaction,
        txid: result.txid.to_string(),
        channel_id: result.channel_id.encode_hex(),
    };
    Ok(Json(response))
}

pub(crate) async fn set_channel_fee(
    macaroon: KndMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Json(channel_fee): Json<ChannelFee>,
) -> Result<impl IntoResponse, StatusCode> {
    handle_unauthorized!(macaroon_auth.verify_admin_macaroon(&macaroon.0));

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
            let (base, ppm) = handle_err!(lightning_interface.set_channel_fee(
                &node_id,
                &channel_ids,
                channel_fee.ppm,
                channel_fee.base
            ));
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
        let (base, ppm) = handle_err!(lightning_interface.set_channel_fee(
            &channel.counterparty.node_id,
            &[channel.channel_id],
            channel_fee.ppm,
            channel_fee.base
        ));
        updated_channels.push(SetChannelFee {
            base,
            ppm,
            peer_id: channel.counterparty.node_id.to_string(),
            channel_id: channel.channel_id.encode_hex(),
            short_channel_id: to_string_empty!(channel.short_channel_id),
        });
    } else {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(SetChannelFeeResponse(updated_channels)))
}

pub(crate) async fn close_channel(
    macaroon: KndMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Path(channel_id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    handle_unauthorized!(macaroon_auth.verify_admin_macaroon(&macaroon.0));

    if let Some(channel) = lightning_interface.list_channels().iter().find(|c| {
        c.channel_id.encode_hex::<String>() == channel_id
            || c.short_channel_id.unwrap_or_default().to_string() == channel_id
    }) {
        handle_err!(
            lightning_interface.close_channel(&channel.channel_id, &channel.counterparty.node_id)
        );
        Ok(Json(()))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
