use std::sync::Arc;

use crate::{handle_err, handle_unauthorized};
use api::Peer;
use axum::{response::IntoResponse, Extension, Json};
use bitcoin::hashes::hex::ToHex;
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
            connected: p.status.to_string(),
            netaddr: p.socked_addr.to_string(),
            alias: p.alias.clone(),
        })
        .collect();

    Ok(Json(peers))
}
