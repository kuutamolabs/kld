use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::anyhow;
use api::{GenerateInvoice, GenerateInvoiceResponse, Invoice, InvoiceStatus, ListInvoiceParams};
use axum::{extract::Query, response::IntoResponse, Extension, Json};
use hex::ToHex;

use crate::{database::millisat_amount::MillisatAmount, ldk::LightningInterface};

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
        payment_hash: invoice.bolt11.payment_hash().encode_hex(),
        expires_at: invoice
            .bolt11
            .expires_at()
            .ok_or_else(|| bad_request(anyhow!("expiry is too far in the future")))?
            .as_secs() as u32,
        bolt11: invoice.bolt11.to_string(),
    };
    Ok(Json(response))
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
            lightning_invoice::InvoiceDescription::Direct(d) => d.to_string(),
            lightning_invoice::InvoiceDescription::Hash(h) => h.0.encode_hex(),
        };
        let amount_received_msat = invoice
            .payments
            .iter()
            .fold(MillisatAmount::zero(), |sum, p| sum + p.amount);
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
            payment_hash: invoice.bolt11.payment_hash().encode_hex(),
            amount_msat: invoice.bolt11.amount_milli_satoshis(),
            status,
            amount_received_msat: if amount_received_msat.0 > 0 {
                Some(amount_received_msat.0)
            } else {
                None
            },
            paid_at: invoice
                .payments
                .first()
                .map(|p| p.timestamp.duration_since(UNIX_EPOCH))
                .transpose()
                .map_err(internal_server)?
                .map(|d| d.as_secs() as u32),
            description,
            expires_at: invoice.bolt11.expires_at().map(|d| d.as_secs()),
        });
    }
    Ok(Json(response))
}
