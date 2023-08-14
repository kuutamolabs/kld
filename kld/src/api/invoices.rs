use std::{
    str::FromStr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::anyhow;
use api::{GenerateInvoice, GenerateInvoiceResponse, Invoice, InvoiceStatus};
use axum::{
    extract::{Path, Query},
    response::IntoResponse,
    Extension, Json,
};
use bitcoin::hashes::hex::ToHex;
use lightning_invoice::Bolt11Invoice;

use super::{
    codegen::get_v1_utility_decode_invoice_string_response::{
        GetV1UtilityDecodeInvoiceStringResponse, GetV1UtilityDecodeInvoiceStringResponseType,
    },
    empty_string_as_none,
};
use crate::{ldk::LightningInterface, MillisatAmount};

use super::{bad_request, internal_server, ApiError};

pub(crate) async fn generate_invoice(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Json(invoice_request): Json<GenerateInvoice>,
) -> Result<impl IntoResponse, ApiError> {
    if invoice_request.label.len() > 100 {
        return Err(bad_request(anyhow!("Label max length is 100 chars")));
    }
    let invoice = lightning_interface
        .generate_invoice(
            invoice_request.label,
            Some(invoice_request.amount),
            invoice_request.description,
            invoice_request.expiry,
        )
        .await
        .map_err(internal_server)?;

    let response = GenerateInvoiceResponse {
        payment_hash: invoice.bolt11.payment_hash().to_hex(),
        expires_at: invoice
            .bolt11
            .expires_at()
            .ok_or_else(|| bad_request(anyhow!("expiry is too far in the future")))?
            .as_secs() as u32,
        bolt11: invoice.bolt11.to_string(),
    };
    Ok(Json(response))
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListInvoiceParams {
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub label: Option<String>,
}

pub(crate) async fn list_invoices(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Query(params): Query<ListInvoiceParams>,
) -> Result<impl IntoResponse, ApiError> {
    if let Some(label) = &params.label {
        if label.len() > 100 {
            return Err(bad_request(anyhow!("Label max length is 100 chars")));
        }
    }
    let mut response = vec![];
    let invoices = lightning_interface
        .list_invoices(params.label)
        .await
        .map_err(internal_server)?;
    for invoice in invoices {
        let description = match invoice.bolt11.description() {
            lightning_invoice::Bolt11InvoiceDescription::Direct(d) => d.to_string(),
            lightning_invoice::Bolt11InvoiceDescription::Hash(h) => h.0.to_hex(),
        };
        let amount_received_msat = invoice
            .payments
            .iter()
            .fold(MillisatAmount::default(), |sum, p| sum + p.amount);
        let status = if !invoice.payments.is_empty() {
            InvoiceStatus::Paid
        } else if invoice.bolt11.expiry_time()
            > SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(internal_server)?
        {
            InvoiceStatus::Expired
        } else {
            InvoiceStatus::Unpaid
        };
        response.push(Invoice {
            label: invoice.label,
            bolt11: invoice.bolt11.to_string(),
            payment_hash: invoice.bolt11.payment_hash().to_hex(),
            amount_msat: invoice.bolt11.amount_milli_satoshis(),
            status,
            amount_received_msat: if amount_received_msat > 0 {
                Some(amount_received_msat)
            } else {
                None
            },
            paid_at: invoice
                .payments
                .first()
                .map(|p| p.timestamp.unix_timestamp() as u32),
            description,
            expires_at: invoice.bolt11.expires_at().map(|d| d.as_secs()),
        });
    }
    Ok(Json(response))
}

pub(crate) async fn decode_invoice(
    Path(maybe_invoice): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    if let Ok(bolt11) = Bolt11Invoice::from_str(&maybe_invoice) {
        return Ok(Json(GetV1UtilityDecodeInvoiceStringResponse {
            type_: GetV1UtilityDecodeInvoiceStringResponseType::Bolt11,
            valid: bolt11.check_signature().is_ok(),
        }));
    }
    Err(bad_request(anyhow!("Invoice could not be decoded")))
}
