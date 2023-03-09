use std::{str::FromStr, sync::Arc};

use crate::api::{bad_request, network::to_api_address, PeerStatus};
use anyhow::Result;
use api::Peer;
use axum::{extract::Path, response::IntoResponse, Extension, Json};
use bitcoin::{hashes::hex::ToHex, secp256k1::PublicKey};

use super::{
    internal_server, unauthorized, ApiError, KldMacaroon, LightningInterface, MacaroonAuth,
};

pub(crate) async fn list_peers(
    macaroon: KldMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, ApiError> {
    macaroon_auth
        .verify_readonly_macaroon(&macaroon.0)
        .map_err(unauthorized)?;

    let peers: Vec<Peer> = lightning_interface
        .list_peers()
        .await
        .map_err(internal_server)?
        .iter()
        .map(|p| Peer {
            id: p.public_key.serialize().to_hex(),
            connected: p.status == PeerStatus::Connected,
            netaddr: p.net_address.as_ref().map(to_api_address),
            alias: p.alias.clone(),
        })
        .collect();

    Ok(Json(peers))
}

pub(crate) async fn connect_peer(
    macaroon: KldMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Json(id): Json<String>,
) -> Result<impl IntoResponse, ApiError> {
    macaroon_auth
        .verify_admin_macaroon(&macaroon.0)
        .map_err(unauthorized)?;

    let (public_key, net_address) = match id.split_once('@') {
        Some((public_key, net_address)) => (
            PublicKey::from_str(public_key).map_err(bad_request)?,
            Some(net_address.parse().map_err(bad_request)?),
        ),
        None => (PublicKey::from_str(&id).map_err(bad_request)?, None),
    };
    lightning_interface
        .connect_peer(public_key, net_address)
        .await
        .map_err(internal_server)?;

    Ok(Json(public_key.serialize().to_hex()))
}

pub(crate) async fn disconnect_peer(
    macaroon: KldMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    macaroon_auth
        .verify_admin_macaroon(&macaroon.0)
        .map_err(unauthorized)?;

    let public_key = PublicKey::from_str(&id).map_err(bad_request)?;
    lightning_interface
        .disconnect_peer(public_key)
        .await
        .map_err(internal_server)?;

    Ok(Json(()))
}
