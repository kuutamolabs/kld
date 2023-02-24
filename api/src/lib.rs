use serde::{Deserialize, Serialize};

pub mod routes {
    /// --- General ---
    /// NO-OP
    pub const ROOT: &str = "/";
    /// Get node information.
    pub const GET_INFO: &str = "/v1/getinfo";
    /// Websocket
    pub const WEBSOCKET: &str = "/v1/ws";

    /// --- Peers ---
    /// Connect with a network peer.
    pub const CONNECT_PEER: &str = "/v1/peer/connect";
    /// Returns the list of peers connected with the node.
    pub const LIST_PEERS: &str = "/v1/peer/listPeers";
    /// Disconnect from a connected network peer.
    pub const DISCONNECT_PEER: &str = "/v1/peer/disconnect";

    /// --- Channels ---
    /// Get the list of channels open on the node.
    pub const LIST_CHANNELS: &str = "/v1/channel/listChannels";
    /// Open channel with a connected peer node.
    pub const OPEN_CHANNEL: &str = "/v1/channel/openChannel";
    /// Update channel fee policy.
    pub const SET_CHANNEL_FEE: &str = "/v1/channel/setChannelFee";
    /// Close an existing channel with a peer.
    pub const CLOSE_CHANNEL: &str = "/v1/channel/closeChannel";

    /// --- Network ---
    /// Look up a node on the network.
    pub const LIST_NODE: &str = "/v1/network/listnode";

    /// --- On chain wallet ---
    /// Returns total, confirmed and unconfirmed on-chain balances.
    pub const GET_BALANCE: &str = "/v1/getbalance";
    /// Generate address for recieving on-chain funds.
    pub const NEW_ADDR: &str = "/v1/newaddr";
    /// Withdraw on-chain funds to an address.
    pub const WITHDRAW: &str = "/v1/withdraw";
}

#[derive(Serialize, Deserialize)]
pub struct GetInfo {
    pub id: String,
    pub alias: String,
    pub color: String,
    pub num_peers: usize,
    pub num_pending_channels: usize,
    pub num_active_channels: usize,
    pub num_inactive_channels: usize,
    #[serde(rename = "blockheight")]
    pub block_height: u64,
    pub synced_to_chain: bool,
    pub testnet: bool,
    pub chains: Vec<Chain>,
    pub version: String,
    pub api_version: String,
    pub network: String,
    pub address: Vec<Address>,
}

#[derive(Serialize, Deserialize)]
pub struct Address {
    #[serde(rename = "type")]
    pub address_type: String,
    pub address: String,
    pub port: u16,
}

#[derive(Serialize, Deserialize)]
pub struct Chain {
    pub chain: String,
    pub network: String,
}

#[derive(Serialize, Deserialize)]
pub struct WalletBalance {
    #[serde(rename = "totalBalance")]
    pub total_balance: u64,
    #[serde(rename = "confBlance")]
    pub conf_balance: u64,
    #[serde(rename = "unconfBalance")]
    pub unconf_balance: u64,
}

#[derive(Serialize, Deserialize)]
pub struct WalletTransfer {
    /// Any Bitcoin accepted type, including bech32
    pub address: String,
    /// Amount to be withdrawn. The string "all" can be used to specify withdrawal of all available funds
    pub satoshis: String,
    /// urgent, normal or slow
    #[serde(rename = "feeRate")]
    pub fee_rate: Option<String>,
    /// minimum number of confirmations that used outputs should have
    #[serde(rename = "minConf")]
    pub min_conf: Option<String>,
    /// Specifies the utxos to be used to fund the channel, as an array of "txid:vout"
    pub utxos: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct WalletTransferResponse {
    /// Transaction
    pub tx: String,
    /// Transaction ID
    pub txid: String,
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

#[derive(Serialize, Deserialize, Clone)]
pub struct ChannelFee {
    // Short channel ID or channel id. It can be "all" for updating all channels.
    pub id: String,
    // Optional value in msats added as base fee to any routed payment.
    pub base: Option<u32>,
    // Optional value that is added proportionally per-millionths to any routed payment volume in satoshi.
    pub ppm: Option<u32>,
}

#[derive(Serialize, Deserialize)]
pub struct SetChannelFee {
    // Base fee in msats.
    pub base: u32,
    // Fee per-millionths
    pub ppm: u32,
    // Peer ID
    pub peer_id: String,
    // Channel ID
    pub channel_id: String,
    // Short channel ID
    pub short_channel_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct SetChannelFeeResponse(pub Vec<SetChannelFee>);

#[derive(Serialize, Deserialize)]
pub struct CloseChannel {
    /// Channel ID of short channel ID
    pub id: String,
}

#[derive(Serialize, Deserialize, Default)]
pub struct NewAddress {
    /// Address type (bech32 only)
    pub address_type: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct NewAddressResponse {
    /// Address
    pub address: String,
}

#[derive(Serialize, Deserialize)]
pub struct Peer {
    pub id: String,
    pub connected: bool,
    pub netaddr: Option<String>,
    pub alias: String,
}

#[derive(Serialize, Deserialize)]
pub struct Node {
    #[serde(rename = "nodeid")]
    pub node_id: String,
    pub alias: String,
    pub color: String,
    pub last_timestamp: u32,
    pub features: String,
    pub addresses: Vec<Address>,
}
