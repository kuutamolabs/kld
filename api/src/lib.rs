use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct GetInfo {
    #[serde(rename = "id")]
    pub identity_pubkey: String,
    pub alias: String,
    pub num_pending_channels: usize,
    pub num_active_channels: usize,
    pub num_inactive_channels: usize,
    pub num_peers: usize,
    #[serde(rename = "blockheight")]
    pub block_height: usize,
    pub synced_to_chain: bool,
    pub testnet: bool,
    pub chains: Vec<Chain>,
    pub version: String,
}

#[derive(Serialize, Deserialize)]
pub struct Chain {
    pub chain: String,
    pub network: String,
}

#[derive(Serialize, Deserialize)]
pub struct Balance {
    #[serde(rename = "totalBalance")]
    pub total_balance: u64,
    #[serde(rename = "confBlance")]
    pub conf_balance: u64,
    #[serde(rename = "unconfBalance")]
    pub unconf_balance: u64,
}

#[derive(Serialize, Deserialize)]
pub struct Channel {
    /// Pub key
    pub id: String,
    /// Peer connection status (true or false)
    pub connected: String,
    // Channel connection status
    pub state: String,
    /// Channel ID
    pub short_channel_id: String,
    /// Channel ID
    pub channel_id: String,
    /// Channel funding transaction
    pub funding_txid: String,
    /// Private channel flag (true or false)
    pub private: String,
    /// Number of msats on our side
    pub msatoshi_to_us: String,
    /// Total msats in the channel
    pub msatoshi_total: String,
    /// Number of msats to push to their side
    pub msatoshi_to_them: String,
    /// Minimum number of msats on their side
    pub their_channel_reserve_satoshis: String,
    /// Minimum number of msats on our side
    pub our_channel_reserve_satoshis: String,
    /// Spendable msats
    pub spendable_msatoshi: String,
    ///
    /// pub funding_allocation_msat: String,
    /// Flag indicating if this peer initiated the channel (0,1)
    pub direction: u8,
    /// Alias of the node
    pub alias: String,
}
