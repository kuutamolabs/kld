use api::{Address, API_VERSION};
use api::{Chain, GetInfo};
use axum::Json;
use axum::{response::IntoResponse, Extension};
use bitcoin::Network;
use std::sync::Arc;

use crate::ldk::LightningInterface;
use crate::VERSION;

use super::MacaroonAuth;
use super::{internal_server, unauthorized};
use super::{ApiError, KldMacaroon};

pub(crate) async fn get_info(
    macaroon: KldMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, ApiError> {
    macaroon_auth
        .verify_readonly_macaroon(&macaroon.0)
        .map_err(unauthorized)?;

    let info = GetInfo {
        id: lightning_interface.identity_pubkey().to_string(),
        alias: lightning_interface.alias(),
        num_pending_channels: lightning_interface.num_pending_channels(),
        num_active_channels: lightning_interface.num_active_channels(),
        num_inactive_channels: lightning_interface.num_inactive_channels(),
        num_peers: lightning_interface.num_peers(),
        block_height: lightning_interface
            .block_height()
            .map_err(internal_server)?,
        synced_to_chain: true,
        testnet: lightning_interface.network() != Network::Bitcoin,
        chains: vec![Chain {
            chain: "bitcoin".to_string(),
            network: lightning_interface.network().to_string(),
        }],
        version: VERSION.to_string(),
        api_version: API_VERSION.to_string(),
        commit_sha: option_env!("GITHUB_SHA").unwrap_or_default().to_string(),
        color: "".to_string(),
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
