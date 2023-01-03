use std::sync::Arc;

use api::Channel;
use axum::{http::StatusCode, response::IntoResponse, Extension, Json};

use crate::hex_utils::hex_str;
use crate::to_string_empty;

use super::KndMacaroon;
use super::LightningInterface;
use super::MacaroonAuth;

pub(crate) async fn list_channels(
    macaroon: KndMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, StatusCode> {
    if macaroon_auth.verify_macaroon(&macaroon.0).is_err() {
        return Err(StatusCode::UNAUTHORIZED);
    }
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
            channel_id: hex_str(&c.channel_id),
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
