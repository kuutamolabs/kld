use api::Address;
use api::{Chain, GetInfo};
use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use bitcoin::Network;
use log::{info, warn};
use std::sync::Arc;

use crate::{handle_err, handle_unauthorized};

use super::KldMacaroon;
use super::LightningInterface;
use super::MacaroonAuth;

pub(crate) async fn get_info(
    macaroon: KldMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, StatusCode> {
    handle_unauthorized!(macaroon_auth.verify_readonly_macaroon(&macaroon.0));

    let info = GetInfo {
        id: lightning_interface.identity_pubkey().to_string(),
        alias: lightning_interface.alias(),
        num_pending_channels: lightning_interface.num_pending_channels(),
        num_active_channels: lightning_interface.num_active_channels(),
        num_inactive_channels: lightning_interface.num_inactive_channels(),
        num_peers: lightning_interface.num_peers(),
        block_height: handle_err!(lightning_interface.block_height()),
        synced_to_chain: true,
        testnet: lightning_interface.network() != Network::Bitcoin,
        chains: vec![Chain {
            chain: "bitcoin".to_string(),
            network: lightning_interface.network().to_string(),
        }],
        version: lightning_interface.version(),
        color: "".to_string(),
        api_version: "0.9.0".to_string(),
        network: lightning_interface.network().to_string(),
        address: lightning_interface
            .addresses()
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
