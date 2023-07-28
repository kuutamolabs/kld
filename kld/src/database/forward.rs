use crate::MillisatAmount;

use lightning::events::HTLCDestination;
use postgres_types::{FromSql, ToSql};
use time::OffsetDateTime;
use tokio_postgres::Row;
use uuid::Uuid;

use super::{microsecond_timestamp, RowExt};

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
            htlc_destination: row.maybe_read_optional("htlc_destination")?,
            timestamp: row.get_timestamp("timestamp"),
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
            count: row.get_u64("count"),
            amount: row.get_u64("amount"),
            fee: row.get_u64("fee"),
        }
    }
}

pub struct TotalForwards {
    pub count: u64,
    pub amount: MillisatAmount,
    pub fee: MillisatAmount,
}
