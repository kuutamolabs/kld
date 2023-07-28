use anyhow::Context;
use bitcoin::secp256k1::PublicKey;
use lightning::{
    chain::transaction::OutPoint,
    events::ClosureReason,
    ln::{channelmanager::ChannelDetails, features::ChannelTypeFeatures},
};
use time::OffsetDateTime;
use tokio_postgres::Row;

use crate::MillisatAmount;

use super::{microsecond_timestamp, RowExt};

#[derive(Debug, PartialEq, Clone)]
pub struct Channel {
    pub id: [u8; 32],
    pub scid: u64,
    pub user_channel_id: u64,
    pub counterparty: PublicKey,
    pub funding_txo: OutPoint,
    pub is_public: bool,
    pub is_outbound: bool,
    pub value: MillisatAmount,
    pub type_features: ChannelTypeFeatures,
    pub open_timestamp: OffsetDateTime,
    pub close_timestamp: Option<OffsetDateTime>,
    pub closure_reason: Option<ClosureReason>,
}

impl TryFrom<ChannelDetails> for Channel {
    type Error = anyhow::Error;

    fn try_from(details: ChannelDetails) -> Result<Self, Self::Error> {
        Ok(Channel {
            id: details.channel_id,
            scid: details.short_channel_id.context("expected scid")?,
            user_channel_id: details.user_channel_id as u64,
            counterparty: details.counterparty.node_id,
            funding_txo: details.funding_txo.context("expected funding txo")?,
            is_public: details.is_public,
            is_outbound: details.is_outbound,
            value: details.channel_value_satoshis,
            type_features: details.channel_type.context("expected channel type")?,
            open_timestamp: microsecond_timestamp(),
            close_timestamp: None,
            closure_reason: None,
        })
    }
}

impl TryFrom<Row> for Channel {
    type Error = anyhow::Error;

    fn try_from(row: Row) -> Result<Self, Self::Error> {
        Ok(Channel {
            id: row.get::<&str, &[u8]>("id").try_into()?,
            scid: row.get_u64("scid"),
            user_channel_id: row.get_u64("user_channel_id"),
            counterparty: PublicKey::from_slice(row.get::<&str, &[u8]>("counterparty"))?,
            funding_txo: row.maybe_read("funding_txo")?,
            is_public: row.get("is_public"),
            is_outbound: row.get("is_outbound"),
            value: row.get_u64("value"),
            type_features: row.maybe_read("type_features")?,
            open_timestamp: row.get_timestamp("open_timestamp"),
            close_timestamp: row.get_timestamp_optional("close_timestamp"),
            closure_reason: row.maybe_read_optional("closure_reason")?,
        })
    }
}
