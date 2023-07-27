use std::io::Cursor;

use crate::{ldk::decode_error, MillisatAmount};

use lightning::{events::HTLCDestination, util::ser::MaybeReadable};
use postgres_types::{FromSql, ToSql};
use time::{OffsetDateTime, PrimitiveDateTime};
use tokio_postgres::Row;
use uuid::Uuid;

use super::microsecond_timestamp;

#[derive(Debug, PartialEq, Clone)]
pub struct Forward {
    pub id: Uuid,
    pub inbound_channel_id: [u8; 32],
    pub outbound_channel_id: Option<[u8; 32]>,
    pub amount: Option<MillisatAmount>,
    pub fee: Option<MillisatAmount>,
    pub status: ForwardStatus,
    pub htlc_destination: Option<HTLCDestination>,
    pub timestamp: OffsetDateTime,
}

impl Forward {
    pub fn success(
        inbound_channel_id: [u8; 32],
        outbound_channel_id: [u8; 32],
        amount: MillisatAmount,
        fee: MillisatAmount,
    ) -> Forward {
        Forward {
            id: Uuid::new_v4(),
            inbound_channel_id,
            outbound_channel_id: Some(outbound_channel_id),
            amount: Some(amount),
            fee: Some(fee),
            status: ForwardStatus::Succeeded,
            htlc_destination: None,
            timestamp: microsecond_timestamp(),
        }
    }

    pub fn failure(inbound_channel_id: [u8; 32], htlc_destination: HTLCDestination) -> Forward {
        Forward {
            id: Uuid::new_v4(),
            inbound_channel_id,
            outbound_channel_id: None,
            amount: None,
            fee: None,
            status: ForwardStatus::Failed,
            htlc_destination: Some(htlc_destination),
            timestamp: microsecond_timestamp(),
        }
    }
}

impl TryFrom<Row> for Forward {
    type Error = anyhow::Error;

    fn try_from(row: Row) -> std::result::Result<Self, Self::Error> {
        let bytes: Option<&[u8]> = row.get("htlc_destination");
        let htlc_destination = if let Some(bytes) = bytes {
            HTLCDestination::read(&mut Cursor::new(bytes)).map_err(decode_error)?
        } else {
            None
        };
        Ok(Forward {
            id: row.get("id"),
            inbound_channel_id: row.get::<&str, &[u8]>("inbound_channel_id").try_into()?,
            outbound_channel_id: row
                .get::<&str, Option<&[u8]>>("outbound_channel_id")
                .map(|x| x.try_into())
                .transpose()?,
            amount: row
                .get::<&str, Option<i64>>("amount")
                .map(|x| x as MillisatAmount),
            fee: row
                .get::<&str, Option<i64>>("fee")
                .map(|x| x as MillisatAmount),
            status: row.get("status"),
            htlc_destination,
            timestamp: row.get::<&str, PrimitiveDateTime>("timestamp").assume_utc(),
        })
    }
}

#[derive(Debug, ToSql, FromSql, PartialEq, Clone, Copy)]
#[postgres(name = "forward_status")]
pub enum ForwardStatus {
    #[postgres(name = "succeeded")]
    Succeeded,
    #[postgres(name = "failed")]
    Failed,
}

impl From<Row> for TotalForwards {
    fn from(row: Row) -> Self {
        TotalForwards {
            count: row.get::<&str, i64>("count") as u64,
            amount: row.get::<&str, i64>("amount") as u64,
            fee: row.get::<&str, i64>("fee") as u64,
        }
    }
}

pub struct TotalForwards {
    pub count: u64,
    pub amount: MillisatAmount,
    pub fee: MillisatAmount,
}
