use std::{str::FromStr, sync::Arc, time::UNIX_EPOCH};

use api::{KeysendRequest, ListPaysParams, PayInvoice, Payment, PaymentResponse};
use axum::{extract::Query, response::IntoResponse, Extension, Json};
use hex::ToHex;
use lightning::routing::gossip::NodeId;

use crate::{
    database::{invoice::Invoice, payment::PaymentDirection},
    ldk::LightningInterface,
};

use super::{bad_request, internal_server, ApiError};

pub(crate) async fn keysend(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Json(keysend_request): Json<KeysendRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let node_id = NodeId::from_str(&keysend_request.pubkey).map_err(bad_request)?;
    let payment = lightning_interface
        .keysend_payment(node_id, keysend_request.amount)
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
    let invoice: Invoice = pay_invoice_request.bolt11.try_into().map_err(bad_request)?;
    let destination = invoice.payee_pub_key.to_string();
    let amount = invoice.amount;
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
        amount_sent_msat: payment.amount,
        payment_preimage: payment
            .preimage
            .map(|i| i.0.encode_hex::<String>())
            .unwrap_or_default(),
        status: payment.status.to_string(),
    };
    Ok(Json(response))
}

pub(crate) async fn list_payments(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Query(params): Query<ListPaysParams>,
) -> Result<impl IntoResponse, ApiError> {
    let invoice = params
        .invoice
        .map(|i| i.try_into())
        .transpose()
        .map_err(bad_request)?;
    let direction = params
        .direction
        .map(|d| PaymentDirection::from_str(&d))
        .transpose()
        .map_err(bad_request)?;
    let payments: Vec<Payment> = lightning_interface
        .list_payments(invoice, direction)
        .await
        .map_err(internal_server)?
        .into_iter()
        .map(|p| Payment {
            bolt11: p.bolt11,
            status: p.status.to_string(),
            payment_preimage: p.preimage.map(|i| i.0.encode_hex()),
            amount_sent_msat: p.amount.to_string(),
        })
        .collect();
    Ok(Json(payments))
}
