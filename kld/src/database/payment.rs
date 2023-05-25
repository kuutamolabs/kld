use std::fmt;

use lightning::ln::{channelmanager::PaymentId, PaymentHash, PaymentPreimage, PaymentSecret};
use postgres_types::{FromSql, ToSql};
use rand::random;

#[derive(Debug, ToSql, FromSql, PartialEq, Clone, Copy)]
#[postgres(name = "payment_status")]
pub enum PaymentStatus {
    #[postgres(name = "pending")]
    Pending,
    #[postgres(name = "succeeded")]
    Succeeded,
    #[postgres(name = "failed")]
    Failed,
}

#[derive(Debug, ToSql, FromSql, PartialEq, Clone, Copy)]
#[postgres(name = "payment_direction")]
pub enum PaymentDirection {
    #[postgres(name = "inbound")]
    Inbound,
    #[postgres(name = "outbound")]
    Outbound,
}

#[derive(Debug, PartialEq)]
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
}

impl Payment {
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
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct MillisatAmount(pub i64);

impl From<u64> for MillisatAmount {
    fn from(value: u64) -> Self {
        MillisatAmount(value as i64)
    }
}

impl fmt::Display for MillisatAmount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
