use std::sync::Arc;

use api::Channel;
use api::FundChannel;
use api::FundChannelResponse;
use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use bitcoin::secp256k1::PublicKey;
use hex::ToHex;
use log::{info, warn};

use crate::handle_auth_err;
use crate::handle_err;
use crate::to_string_empty;

use super::KndMacaroon;
use super::LightningInterface;
use super::MacaroonAuth;

pub(crate) async fn list_channels(
    macaroon: KndMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, StatusCode> {
    handle_auth_err!(macaroon_auth.verify_readonly_macaroon(&macaroon.0))?;

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
            direction: if c.is_outbound { 1 } else { 0 },
            alias: lightning_interface.get_node(c.counterparty.node_id).map_or(
                "".to_string(),
                |n| {
                    n.announcement_info
                        .map_or("".to_string(), |a| a.alias.to_string())
                },
            ),
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
    handle_auth_err!(macaroon_auth.verify_admin_macaroon(&macaroon.0))?;

    let pub_key_bytes = hex::decode(fund_channel.id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let public_key = PublicKey::from_slice(&pub_key_bytes).map_err(|_| StatusCode::BAD_REQUEST)?;
    let value = fund_channel
        .satoshis
        .parse()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let push_msat = fund_channel
        .push_msat
        .map(|x| x.parse::<u64>().map_err(|_| StatusCode::BAD_REQUEST))
        .transpose()?;

    let result = handle_err!(
        lightning_interface
            .open_channel(public_key, value, push_msat, None)
            .await
    )?;
    let transaction = handle_err!(serde_json::to_string(&result.transaction))?;
    let response = FundChannelResponse {
        tx: transaction,
        txid: result.txid.to_string(),
        channel_id: result.channel_id.encode_hex(),
    };
    Ok(Json(response))
}
