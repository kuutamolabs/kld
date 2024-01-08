use std::{str::FromStr, sync::Arc};

use super::payloads::{KeysendRequest, PayInvoice, PaymentResponse};
use anyhow::Context;
use axum::{extract::Query, response::IntoResponse, Extension, Json};
use lightning::routing::gossip::NodeId;

use crate::{
    database::{
        invoice::Invoice,
        payment::{PaymentDirection, PaymentStatus},
    },
    ldk::LightningInterface,
};

use super::{
    bad_request,
    codegen::get_v1_pay_list_payments_response::{
        GetV1PayListPaymentsResponse, GetV1PayListPaymentsResponsePaymentsItem,
        GetV1PayListPaymentsResponsePaymentsItemStatus,
    },
    empty_string_as_none, internal_server, ApiError,
};

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
        payment_hash: payment
            .hash
            .context("missing payment hash")
            .map_err(internal_server)?
            .0
            .to_hex(),
        created_at: payment.timestamp.unix_timestamp() as u64,
        parts: 1,
        amount_msat: Some(keysend_request.amount),
        amount_sent_msat: keysend_request.amount * 1000,
        payment_preimage: payment.preimage.map(|i| i.0.to_hex()).unwrap_or_default(),
        status: payment.status.to_string(),
    };
    Ok(Json(response))
}

pub(crate) async fn pay_invoice(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Json(pay_invoice_request): Json<PayInvoice>,
) -> Result<impl IntoResponse, ApiError> {
    let invoice: Invoice = pay_invoice_request
        .invoice
        .try_into()
        .map_err(bad_request)?;
    let destination = invoice.payee_pub_key.to_string();
    let amount = invoice.amount;
    let payment = lightning_interface
        .pay_invoice(invoice, pay_invoice_request.label)
        .await
        .map_err(internal_server)?;
    let response = PaymentResponse {
        destination,
        payment_hash: payment
            .hash
            .context("missing payment hash")
            .map_err(internal_server)?
            .0
            .to_hex(),
        created_at: payment.timestamp.unix_timestamp() as u64,
        parts: 1,
        amount_msat: amount,
        amount_sent_msat: payment.amount,
        payment_preimage: payment.preimage.map(|i| i.0.to_hex()).unwrap_or_default(),
        status: payment.status.to_string(),
    };
    Ok(Json(response))
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPaysParams {
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub invoice: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub direction: Option<String>,
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
    let payments: Vec<GetV1PayListPaymentsResponsePaymentsItem> = lightning_interface
        .list_payments(invoice, direction)
        .await
        .map_err(internal_server)?
        .into_iter()
        .map(|p| GetV1PayListPaymentsResponsePaymentsItem {
            bolt11: p.bolt11.as_ref().map(|b| b.to_string()),
            status: match p.status {
                PaymentStatus::Pending => GetV1PayListPaymentsResponsePaymentsItemStatus::Pending,
                PaymentStatus::Succeeded => {
                    GetV1PayListPaymentsResponsePaymentsItemStatus::Complete
                }
                _ => GetV1PayListPaymentsResponsePaymentsItemStatus::Failed,
            },
            payment_preimage: p.preimage.map(|i| i.0.to_hex()),
            amount_sent_msat: p.amount,
            amount_msat: p.bolt11.as_ref().and_then(|b| b.amount_milli_satoshis()),
            created_at: p.timestamp.unix_timestamp() as u64,
            destination: p
                .bolt11
                .and_then(|b| b.payee_pub_key().map(|pk| pk.to_string())),
            id: p.id.0.to_hex(),
            memo: p.label,
            payment_hash: p.hash.map(|h| h.0.to_hex()),
        })
        .collect();
    Ok(Json(GetV1PayListPaymentsResponse { payments }))
}
