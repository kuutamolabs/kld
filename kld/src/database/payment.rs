use anyhow::{Context, Result};
use bitcoin::hashes::Hash;
use lightning::{
    events::PaymentFailureReason,
    ln::{channelmanager::PaymentId, PaymentHash, PaymentPreimage, PaymentSecret},
};
use lightning_invoice::Bolt11Invoice;
use postgres_types::{FromSql, ToSql};
use rand::random;
use std::{
    fmt::{self, Display},
    str::FromStr,
};
use thiserror::Error;
use time::OffsetDateTime;
use tokio_postgres::Row;

use crate::MillisatAmount;

use super::{invoice::Invoice, microsecond_timestamp, RowExt};

#[derive(Debug, ToSql, FromSql, PartialEq, Clone, Copy)]
#[postgres(name = "payment_status")]
pub enum PaymentStatus {
    #[postgres(name = "pending")]
    Pending,
    #[postgres(name = "succeeded")]
    Succeeded,
    #[postgres(name = "recipient_rejected")]
    RecipientRejected,
    #[postgres(name = "user_abandoned")]
    UserAbandoned,
    #[postgres(name = "retries_exhausted")]
    RetriesExhausted,
    #[postgres(name = "expired")]
    Expired,
    #[postgres(name = "route_not_found")]
    RouteNotFound,
    #[postgres(name = "error")]
    Error,
}

impl Display for PaymentStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PaymentStatus::Pending => f.write_str("pending"),
            PaymentStatus::Succeeded => f.write_str("succeeded"),
            PaymentStatus::RecipientRejected => f.write_str("recipient rejected"),
            PaymentStatus::UserAbandoned => f.write_str("user abandoned"),
            PaymentStatus::RetriesExhausted => f.write_str("retries exhausted"),
            PaymentStatus::Expired => f.write_str("expired"),
            PaymentStatus::RouteNotFound => f.write_str("route not found"),
            PaymentStatus::Error => f.write_str("error"),
        }
    }
}

#[derive(Debug, ToSql, FromSql, PartialEq, Clone, Copy)]
#[postgres(name = "payment_direction")]
pub enum PaymentDirection {
    #[postgres(name = "inbound")]
    Inbound,
    #[postgres(name = "outbound")]
    Outbound,
}

#[derive(Error, Debug)]
pub enum DeserializeError {
    #[error("unable to deserialize {0}")]
    PaymentDirection(String),
}

impl Display for PaymentDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaymentDirection::Inbound => f.write_str("inbound"),
            PaymentDirection::Outbound => f.write_str("outbound"),
        }
    }
}

impl FromStr for PaymentDirection {
    type Err = DeserializeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "inbound" => Ok(PaymentDirection::Inbound),
            "outbound" => Ok(PaymentDirection::Outbound),
            _ => Err(DeserializeError::PaymentDirection(s.to_string())),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Payment {
    pub id: PaymentId,
    // Hash of the premimage.
    pub hash: PaymentHash,
    pub preimage: Option<PaymentPreimage>,
    // No secret indicates a spontaneous payment.
    pub secret: Option<PaymentSecret>,
    pub label: Option<String>,
    pub status: PaymentStatus,
    pub amount: MillisatAmount,
    pub fee: Option<MillisatAmount>,
    pub direction: PaymentDirection,
    // The time that the payment was sent/received.
    pub timestamp: OffsetDateTime,
    // The bolt11 invoice with corresponding payment hash. Useful when querying payments.
    pub bolt11: Option<Bolt11Invoice>,
}

impl Payment {
    pub fn new_id() -> PaymentId {
        PaymentId(random())
    }

    pub fn spontaneous_inbound(
        hash: PaymentHash,
        preimage: PaymentPreimage,
        amount: MillisatAmount,
    ) -> Self {
        Payment {
            id: PaymentId(random()),
            hash,
            preimage: Some(preimage),
            secret: None,
            label: None,
            status: PaymentStatus::Pending,
            amount,
            fee: None,
            direction: PaymentDirection::Inbound,
            timestamp: microsecond_timestamp(),
            bolt11: None,
        }
    }

    pub fn spontaneous_outbound(id: PaymentId, hash: PaymentHash, amount: MillisatAmount) -> Self {
        Payment {
            id,
            hash,
            preimage: None,
            secret: None,
            label: None,
            status: PaymentStatus::Pending,
            amount,
            fee: None,
            direction: PaymentDirection::Outbound,
            timestamp: microsecond_timestamp(),
            bolt11: None,
        }
    }

    pub fn of_invoice_inbound(
        hash: PaymentHash,
        preimage: Option<PaymentPreimage>,
        secret: PaymentSecret,
        amount: MillisatAmount,
    ) -> Self {
        Payment {
            id: PaymentId(random()),
            hash,
            preimage,
            secret: Some(secret),
            label: None,
            status: PaymentStatus::Succeeded,
            amount,
            fee: None,
            direction: PaymentDirection::Inbound,
            timestamp: microsecond_timestamp(),
            bolt11: None,
        }
    }

    pub fn of_invoice_outbound(invoice: &Invoice, label: Option<String>) -> Self {
        Payment {
            id: PaymentId(random()),
            hash: PaymentHash(*invoice.bolt11.payment_hash().as_inner()),
            preimage: None,
            secret: Some(*invoice.bolt11.payment_secret()),
            label,
            status: PaymentStatus::Pending,
            amount: invoice.bolt11.amount_milli_satoshis().unwrap_or_default(),
            fee: None,
            direction: PaymentDirection::Outbound,
            timestamp: microsecond_timestamp(),
            bolt11: Some(invoice.bolt11.clone()),
        }
    }

    pub fn succeeded(&mut self, preimage: Option<PaymentPreimage>, fee: Option<MillisatAmount>) {
        self.preimage = preimage;
        self.fee = fee;
        self.status = PaymentStatus::Succeeded;
    }

    pub fn failed(&mut self, reason: Option<PaymentFailureReason>) {
        self.status = match reason {
            Some(PaymentFailureReason::RecipientRejected) => PaymentStatus::RecipientRejected,
            Some(PaymentFailureReason::UserAbandoned) => PaymentStatus::UserAbandoned,
            Some(PaymentFailureReason::RetriesExhausted) => PaymentStatus::RetriesExhausted,
            Some(PaymentFailureReason::PaymentExpired) => PaymentStatus::Expired,
            Some(PaymentFailureReason::RouteNotFound) => PaymentStatus::RouteNotFound,
            _ => PaymentStatus::Error,
        };
    }
}

impl TryFrom<&Row> for Payment {
    type Error = anyhow::Error;

    fn try_from(row: &Row) -> std::result::Result<Self, Self::Error> {
        let id: &[u8] = row.get("id");
        let hash: &[u8] = row.get("hash");
        let preimage: Option<&[u8]> = row.get("preimage");
        let secret: Option<&[u8]> = row.get("secret");
        let label: Option<String> = row.get("label");

        let preimage = match preimage {
            Some(bytes) => Some(PaymentPreimage(bytes.try_into().context("bad preimage")?)),
            None => None,
        };
        let secret = match secret {
            Some(bytes) => Some(PaymentSecret(bytes.try_into().context("bad secret")?)),
            None => None,
        };

        Ok(Payment {
            id: PaymentId(id.try_into().context("bad ID")?),
            hash: PaymentHash(hash.try_into().context("bad hash")?),
            preimage,
            secret,
            label,
            status: row.get("status"),
            amount: row.get::<&str, i64>("amount") as MillisatAmount,
            fee: row
                .get::<&str, Option<i64>>("fee")
                .map(|f| f as MillisatAmount),
            direction: row.get("direction"),
            timestamp: row.get_timestamp("timestamp"),
            bolt11: Bolt11Invoice::from_str(row.get("bolt11")).ok(),
        })
    }
}
