use std::{str::FromStr, sync::Arc, time::UNIX_EPOCH};

use api::lightning::routing::gossip::NodeId;
use api::{KeysendRequest, PayInvoice, PaymentResponse};
use axum::{response::IntoResponse, Extension, Json};
use hex::ToHex;

use crate::{
    database::{invoice::Invoice, payment::MillisatAmount},
    ldk::LightningInterface,
};

use super::{bad_request, internal_server, ApiError};

pub(crate) async fn keysend(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Json(keysend_request): Json<KeysendRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let node_id = NodeId::from_str(&keysend_request.pubkey).map_err(bad_request)?;
    let amount = MillisatAmount(keysend_request.amount);
    let payment = lightning_interface
        .keysend_payment(node_id, amount)
        .await
        .map_err(internal_server)?;
    let response = PaymentResponse {
        destination: keysend_request.pubkey,
        payment_hash: payment.hash.0.encode_hex::<String>(),
        created_at: payment
            .timestamp
            .duration_since(UNIX_EPOCH)
            .map_err(internal_server)?
            .as_secs(),
        parts: 1,
        amount_msat: Some(keysend_request.amount),
        amount_sent_msat: keysend_request.amount * 1000,
        payment_preimage: payment
            .preimage
            .map(|i| i.0.encode_hex::<String>())
            .unwrap_or_default(),
        status: payment.status.to_string(),
    };
    Ok(Json(response))
}

pub(crate) async fn pay_invoice(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Json(pay_invoice_request): Json<PayInvoice>,
) -> Result<impl IntoResponse, ApiError> {
    let bolt11 =
        lightning_invoice::Invoice::from_str(&pay_invoice_request.bolt11).map_err(bad_request)?;
    bolt11.check_signature().map_err(bad_request)?;
    let invoice = Invoice::new(None, bolt11).map_err(bad_request)?;
    let destination = invoice.payee_pub_key.to_string();
    let amount = invoice.amount.map(|a| a.0);
    let payment = lightning_interface
        .pay_invoice(invoice, pay_invoice_request.label)
        .await
        .map_err(internal_server)?;
    let response = PaymentResponse {
        destination,
        payment_hash: payment.hash.0.encode_hex::<String>(),
        created_at: payment
            .timestamp
            .duration_since(UNIX_EPOCH)
            .map_err(internal_server)?
            .as_secs(),
        parts: 1,
        amount_msat: amount,
        amount_sent_msat: payment.amount.0,
        payment_preimage: payment
            .preimage
            .map(|i| i.0.encode_hex::<String>())
            .unwrap_or_default(),
        status: payment.status.to_string(),
    };
    Ok(Json(response))
}
