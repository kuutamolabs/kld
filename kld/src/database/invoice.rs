use std::{str::FromStr, time::SystemTime};

use anyhow::{anyhow, Result};
use api::lightning::ln::PaymentHash;
use bitcoin::{hashes::Hash, secp256k1::PublicKey};

use crate::MillisatAmount;

use super::payment::Payment;

#[derive(Clone, Debug, PartialEq)]
pub struct Invoice {
    pub payment_hash: PaymentHash,
    // User generated id for the invoice.
    pub label: Option<String>,
    pub bolt11: lightning_invoice::Invoice,
    // None if we are the payee.
    pub payee_pub_key: PublicKey,
    pub expiry: Option<u64>,
    pub amount: Option<MillisatAmount>,
    // The time that the invoice was generated.
    pub timestamp: SystemTime,
    // Payments with the payment_hash of the bolt11 invoice.
    pub payments: Vec<Payment>,
}

impl TryFrom<String> for Invoice {
    type Error = anyhow::Error;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        let bolt11 = lightning_invoice::Invoice::from_str(&value)?;
        bolt11.check_signature()?;
        Invoice::new(None, bolt11)
    }
}

impl Invoice {
    pub fn new(label: Option<String>, bolt11: lightning_invoice::Invoice) -> Result<Self> {
        let raw = bolt11.clone().into_signed_raw();
        let expiry = raw.expiry_time().map(|t| t.as_seconds());
        let payee_pub_key = raw.recover_payee_pub_key()?.0;
        let amount = raw.amount_pico_btc().map(|a| a / 10);
        let timestamp = bolt11.timestamp();
        Ok(Invoice {
            payment_hash: PaymentHash(
                raw.payment_hash()
                    .ok_or_else(|| anyhow!("missing payment hash"))?
                    .0
                    .into_inner(),
            ),
            label,
            bolt11,
            payee_pub_key,
            expiry,
            amount,
            timestamp,
            payments: vec![],
        })
    }

    pub fn deserialize(
        payment_hash: PaymentHash,
        label: Option<String>,
        bolt11: String,
        payee_pub_key: Vec<u8>,
        expiry: Option<u64>,
        amount: Option<i64>,
        timestamp: SystemTime,
    ) -> Result<Self> {
        Ok(Invoice {
            payment_hash,
            label,
            bolt11: lightning_invoice::Invoice::from_str(&bolt11)?,
            payee_pub_key: PublicKey::from_slice(&payee_pub_key)?,
            expiry,
            amount: amount.map(|a| a as u64),
            timestamp,
            payments: vec![],
        })
    }
}
