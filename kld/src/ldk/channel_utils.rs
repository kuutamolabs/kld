/// Maximum transaction index that can be used in a `short_channel_id`.
/// This value is based on the 3-bytes available for tx index.
pub const MAX_SCID_TX_INDEX: u64 = 0x00ffffff;

/// Maximum vout index that can be used in a `short_channel_id`. This
/// value is based on the 2-bytes available for the vout index.
pub const MAX_SCID_VOUT_INDEX: u64 = 0xffff;

/// Extracts the block height (most significant 3-bytes) from the `short_channel_id`
pub fn block_from_scid(short_channel_id: &u64) -> u32 {
    (short_channel_id >> 40) as u32
}

/// Extracts the tx index (bytes [2..4]) from the `short_channel_id`
pub fn tx_index_from_scid(short_channel_id: &u64) -> u32 {
    ((short_channel_id >> 16) & MAX_SCID_TX_INDEX) as u32
}

/// Extracts the vout (bytes [0..2]) from the `short_channel_id`
pub fn vout_from_scid(short_channel_id: &u64) -> u16 {
    ((short_channel_id) & MAX_SCID_VOUT_INDEX) as u16
}
