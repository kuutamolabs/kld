use bitcoin::hashes::Hash;
use lightning::{
    events::PaymentFailureReason,
    ln::{channelmanager::PaymentId, PaymentHash, PaymentPreimage, PaymentSecret},
};
use postgres_types::{FromSql, ToSql};
use rand::random;
use std::{
    fmt::{self, Display},
    str::FromStr,
    time::SystemTime,
};
use thiserror::Error;

use crate::MillisatAmount;

use super::invoice::Invoice;

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
    pub timestamp: SystemTime,
    // The bolt11 invoice with corresponding payment hash. Useful when querying payments.
    pub bolt11: Option<String>,
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
            timestamp: SystemTime::now(),
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
            timestamp: SystemTime::now(),
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
            timestamp: SystemTime::now(),
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
            timestamp: SystemTime::now(),
            bolt11: Some(invoice.bolt11.to_string()),
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
