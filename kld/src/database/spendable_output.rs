use std::io::Cursor;

use anyhow::Result;
use bitcoin::{hashes::Hash, Txid};
use lightning::{
    sign::SpendableOutputDescriptor,
    util::ser::{Readable, Writeable},
};
use postgres_types::{FromSql, ToSql};
use tokio_postgres::Row;

#[derive(Clone, Debug)]
pub struct SpendableOutput {
    pub txid: Txid,
    pub vout: u16,
    pub value: u64,
    pub descriptor: SpendableOutputDescriptor,
    pub status: SpendableOutputStatus,
}

impl SpendableOutput {
    pub fn new(descriptor: SpendableOutputDescriptor) -> Self {
        let (txid, vout, value) = match &descriptor {
            SpendableOutputDescriptor::StaticOutput { outpoint, output } => {
                (outpoint.txid, outpoint.index, output.value)
            }
            SpendableOutputDescriptor::DelayedPaymentOutput(descriptor) => (
                descriptor.outpoint.txid,
                descriptor.outpoint.index,
                descriptor.output.value,
            ),
            SpendableOutputDescriptor::StaticPaymentOutput(descriptor) => (
                descriptor.outpoint.txid,
                descriptor.outpoint.index,
                descriptor.output.value,
            ),
        };
        SpendableOutput {
            txid,
            vout,
            value,
            descriptor,
            status: SpendableOutputStatus::Unspent,
        }
    }

    pub fn from_row(row: Row) -> Result<SpendableOutput> {
        let bytes: Vec<u8> = row.get("descriptor");
        let descriptor = SpendableOutputDescriptor::read(&mut Cursor::new(bytes)).unwrap();
        let bytes: Vec<u8> = row.get("txid");
        Ok(SpendableOutput {
            txid: Txid::from_slice(&bytes)?,
            vout: row.get::<&str, i64>("vout") as u16,
            value: row.get::<&str, i64>("value") as u64,
            descriptor,
            status: row.get("status"),
        })
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut bytes = vec![];
        self.descriptor.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, ToSql, FromSql, PartialEq, Clone, Copy)]
#[postgres(name = "spendable_output_status")]
pub enum SpendableOutputStatus {
    #[postgres(name = "unspent")]
    Unspent,
    #[postgres(name = "spent")]
    Spent,
}
