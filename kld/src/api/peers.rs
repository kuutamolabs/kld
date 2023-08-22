use std::{str::FromStr, sync::Arc};

use super::codegen::{
    post_v1_peer_connect_body::PostV1PeerConnectBody,
    post_v1_peer_connect_response::PostV1PeerConnectResponse,
};
use crate::{
    api::bad_request,
    ldk::{LightningInterface, PeerStatus},
};
use anyhow::Result;
use api::Peer;
use axum::{extract::Path, response::IntoResponse, Extension, Json};
use bitcoin::{hashes::hex::ToHex, secp256k1::PublicKey};
use http::StatusCode;

use super::{internal_server, ApiError};

pub(crate) async fn list_peers(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, ApiError> {
    let peers: Vec<Peer> = lightning_interface
        .list_peers()
        .await
        .map_err(internal_server)?
        .iter()
        .map(|p| Peer {
            id: p.public_key.serialize().to_hex(),
            connected: p.status == PeerStatus::Connected,
            netaddr: p.net_address.as_ref().map(|a| a.to_string()),
            alias: p.alias.clone(),
        })
        .collect();

    Ok(Json(peers))
}

pub(crate) async fn connect_peer(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Json(body): Json<PostV1PeerConnectBody>,
) -> Result<impl IntoResponse, ApiError> {
    let (public_key, net_address) = match body.id.split_once('@') {
        Some((public_key, net_address)) => (
            PublicKey::from_str(public_key).map_err(bad_request)?,
            Some(net_address.parse().map_err(bad_request)?),
        ),
        None => (PublicKey::from_str(&body.id).map_err(bad_request)?, None),
    };
    lightning_interface
        .connect_peer(public_key, net_address)
        .await
        .map_err(internal_server)?;

    Ok((
        StatusCode::CREATED,
        Json(PostV1PeerConnectResponse {
            id: public_key.serialize().to_hex(),
        }),
    ))
}

pub(crate) async fn disconnect_peer(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let public_key = PublicKey::from_str(&id).map_err(bad_request)?;
    lightning_interface
        .disconnect_peer(public_key)
        .await
        .map_err(internal_server)?;

    Ok(Json(()))
}
