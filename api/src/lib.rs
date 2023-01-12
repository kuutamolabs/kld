use serde::{Deserialize, Serialize};

pub mod routes {
    pub const ROOT: &str = "/";
    pub const GET_INFO: &str = "/v1/getinfo";
    pub const GET_BALANCE: &str = "/v1/getbalance";
    pub const LIST_CHANNELS: &str = "/v1/channel/listChannels";
    pub const OPEN_CHANNEL: &str = "/v1/channel/openChannel";
}

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

#[derive(Serialize, Deserialize)]
pub struct FundChannel {
    /// Pub key of the peer
    pub id: String,
    /// Amount in satoshis
    pub satoshis: String,
    /// urgent/normal/slow/<sats>perkw/<sats>perkb
    #[serde(rename = "feeRate")]
    pub fee_rate: Option<String>,
    /// Flag to announce the channel (true, false)
    /// Default: 'true'
    pub announce: Option<String>,
    /// Minimum number of confirmations that used outputs should have
    #[serde(rename = "minConf")]
    pub min_conf: Option<u8>,
    /// Specifies the utxos to be used to fund the channel, as an array of "txid:vout"
    pub utxos: Vec<String>,
    /// Amount of millisatoshis to push to the channel peer at open
    pub push_msat: Option<String>,
    /// Bitcoin address to which the channel funds should be sent to on close
    pub close_to: Option<String>,
    /// Amount of liquidity you'd like to lease from the peer
    pub request_amt: Option<String>,
    /// Compact represenation of the peer's expected channel lease terms
    pub compact_lease: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct FundChannelResponse {
    /// Transaction
    pub tx: String,
    /// Transaction ID
    pub txid: String,
    /// channel_id of the newly created channel (hex)
    pub channel_id: String,
}
