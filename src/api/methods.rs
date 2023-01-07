use api::{Chain, GetInfo};
use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use bitcoin::Network;
use log::info;
use std::sync::Arc;

use crate::handle_auth_err;

use super::KndMacaroon;
use super::LightningInterface;
use super::MacaroonAuth;

pub(crate) async fn get_info(
    macaroon: KndMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, StatusCode> {
    handle_auth_err!(macaroon_auth.verify_readonly_macaroon(&macaroon.0))?;

    let info = GetInfo {
        identity_pubkey: lightning_interface.identity_pubkey().to_string(),
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
            network: lightning_interface.network().to_string(),
        }],
        version: lightning_interface.version(),
    };
    Ok(Json(info))
}
