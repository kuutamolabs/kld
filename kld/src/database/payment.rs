use std::{
    fmt::{self, Display},
    time::SystemTime,
};

use lightning::{
    events::PaymentFailureReason,
    ln::{channelmanager::PaymentId, PaymentHash, PaymentPreimage, PaymentSecret},
};
use postgres_types::{FromSql, ToSql};
use rand::random;

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

#[derive(Clone, Debug, PartialEq)]
pub struct Payment {
    pub id: PaymentId,
    // Hash of the premimage.
    pub hash: PaymentHash,
    pub preimage: Option<PaymentPreimage>,
    // No secret indicates a spontaneous payment.
    pub secret: Option<PaymentSecret>,
    pub status: PaymentStatus,
    pub amount: MillisatAmount,
    pub fee: Option<MillisatAmount>,
    pub direction: PaymentDirection,
    pub timestamp: SystemTime,
}

impl Payment {
    pub fn generate_id() -> PaymentId {
        PaymentId(random())
    }

    pub fn new_spontaneous_inbound(
        hash: PaymentHash,
        preimage: PaymentPreimage,
        amount: MillisatAmount,
    ) -> Payment {
        Payment {
            id: PaymentId(random()),
            hash,
            preimage: Some(preimage),
            secret: None,
            status: PaymentStatus::Pending,
            amount,
            fee: None,
            direction: PaymentDirection::Inbound,
            timestamp: SystemTime::now(),
        }
    }

    pub fn new_spontaneous_outbound(hash: PaymentHash, amount: MillisatAmount) -> Payment {
        Payment {
            id: PaymentId(random()),
            hash,
            preimage: None,
            secret: None,
            status: PaymentStatus::Pending,
            amount,
            fee: None,
            direction: PaymentDirection::Outbound,
            timestamp: SystemTime::now(),
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

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct MillisatAmount(pub u64);

impl MillisatAmount {
    pub fn as_i64(&self) -> i64 {
        self.0 as i64
    }
}

impl From<i64> for MillisatAmount {
    fn from(value: i64) -> Self {
        MillisatAmount(value as u64)
    }
}

impl fmt::Display for MillisatAmount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
