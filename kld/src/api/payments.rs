use std::{str::FromStr, sync::Arc, time::UNIX_EPOCH};

use api::{KeysendRequest, KeysendResponse};
use axum::{response::IntoResponse, Extension, Json};
use hex::ToHex;
use lightning::routing::gossip::NodeId;

use crate::{database::payment::MillisatAmount, ldk::LightningInterface};

use super::{bad_request, internal_server, ApiError};

pub(crate) async fn keysend(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Json(keysend_request): Json<KeysendRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let node_id = NodeId::from_str(&keysend_request.pubkey).map_err(bad_request)?;
    let amount = MillisatAmount(keysend_request.amount);
    let payment = lightning_interface
        .send_payment(node_id, amount)
        .await
        .map_err(internal_server)?;
    let response = KeysendResponse {
        destination: keysend_request.pubkey,
        payment_hash: payment.hash.0.encode_hex::<String>(),
        created_at: payment
            .timestamp
            .duration_since(UNIX_EPOCH)
            .map_err(internal_server)?
            .as_secs(),
        parts: 1,
        amount_msat: keysend_request.amount,
        amount_sent_msat: keysend_request.amount * 1000,
        payment_preimage: payment
            .preimage
            .map(|i| i.0.encode_hex::<String>())
            .unwrap_or_default(),
        status: payment.status.to_string(),
    };
    Ok(Json(response))
}
