use std::io::Cursor;

use anyhow::Result;
use lightning::{
    chain::keysinterface::SpendableOutputDescriptor,
    util::ser::{Readable, Writeable},
};
use postgres_types::{FromSql, ToSql};
use tokio_postgres::Row;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct SpendableOutput {
    pub id: Uuid,
    pub descriptor: SpendableOutputDescriptor,
    pub status: SpendableOutputStatus,
}

impl SpendableOutput {
    pub fn new(descriptor: SpendableOutputDescriptor) -> Self {
        SpendableOutput {
            id: Uuid::new_v4(),
            descriptor,
            status: SpendableOutputStatus::Unspent,
        }
    }

    pub fn from_row(row: Row) -> Result<SpendableOutput> {
        let bytes: Vec<u8> = row.get("descriptor");
        let descriptor = SpendableOutputDescriptor::read(&mut Cursor::new(bytes)).unwrap();
        Ok(SpendableOutput {
            id: row.get("id"),
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
