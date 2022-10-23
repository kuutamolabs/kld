use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, Mutex},
};

use lightning::ln::{PaymentHash, PaymentPreimage, PaymentSecret};

pub(crate) enum HTLCStatus {
    Succeeded,
    Failed,
}

pub(crate) struct PaymentInfo {
    pub preimage: Option<PaymentPreimage>,
    pub secret: Option<PaymentSecret>,
    pub status: HTLCStatus,
    pub amt_msat: MillisatAmount,
}

pub(crate) struct MillisatAmount(pub Option<u64>);

impl fmt::Display for MillisatAmount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            Some(amt) => write!(f, "{}", amt),
            None => write!(f, "unknown"),
        }
    }
}

pub(crate) type PaymentInfoStorage = Arc<Mutex<HashMap<PaymentHash, PaymentInfo>>>;
