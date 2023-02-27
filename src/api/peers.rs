use std::{net::SocketAddr, str::FromStr, sync::Arc};

use crate::{api::PeerStatus, handle_err, handle_unauthorized};
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
            netaddr: p.socket_addr.map(|s| s.to_string()),
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

    let (public_key, socket_addr) = match id.split_once('@') {
        Some((public_key, socket_addr)) => (
            handle_err!(PublicKey::from_str(public_key)),
            Some(handle_err!(SocketAddr::from_str(socket_addr))),
        ),
        None => (handle_err!(PublicKey::from_str(&id)), None),
    };
    handle_err!(
        lightning_interface
            .connect_peer(public_key, socket_addr)
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
