use std::{str::FromStr, sync::Arc};

use crate::{
    api::{network::to_api_address, PeerStatus},
    handle_err, handle_unauthorized,
    net_utils::parse_net_address,
};
use anyhow::Result;
use api::Peer;
use axum::{extract::Path, response::IntoResponse, Extension, Json};
use bitcoin::{hashes::hex::ToHex, secp256k1::PublicKey};
use hyper::StatusCode;
use log::{info, warn};

use super::{KndMacaroon, LightningInterface, MacaroonAuth};

pub(crate) async fn list_peers(
    macaroon: KndMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, StatusCode> {
    handle_unauthorized!(macaroon_auth.verify_readonly_macaroon(&macaroon.0));

    let peers: Vec<Peer> = handle_err!(lightning_interface.list_peers().await)
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
    macaroon: KndMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Json(id): Json<String>,
) -> Result<impl IntoResponse, StatusCode> {
    handle_unauthorized!(macaroon_auth.verify_admin_macaroon(&macaroon.0));

    let (public_key, net_address) = match id.split_once('@') {
        Some((public_key, net_address)) => (
            handle_err!(PublicKey::from_str(public_key)),
            Some(handle_err!(parse_net_address(net_address))),
        ),
        None => (handle_err!(PublicKey::from_str(&id)), None),
    };
    handle_err!(
        lightning_interface
            .connect_peer(public_key, net_address)
            .await
    );

    Ok(Json(public_key.serialize().to_hex()))
}

pub(crate) async fn disconnect_peer(
    macaroon: KndMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    handle_unauthorized!(macaroon_auth.verify_admin_macaroon(&macaroon.0));

    let public_key = handle_err!(PublicKey::from_str(&id));
    handle_err!(lightning_interface.disconnect_peer(public_key).await);

    Ok(Json(()))
}
