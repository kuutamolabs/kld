use std::{fmt::Display, str::FromStr};

use bitcoin::Transaction;
use serde::{de::Visitor, Deserialize, Serialize};

pub const API_VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod routes {
    /// --- General ---
    /// NO-OP
    pub const ROOT: &str = "/";
    /// Sign
    pub const SIGN: &str = "/v1/utility/signMessage";
    /// Get node information.
    pub const GET_INFO: &str = "/v1/getinfo";
    /// Get node routing fees.
    pub const GET_FEES: &str = "/v1/getFees";
    /// Estimate channel liquidity range to a particular node.
    pub const ESTIMATE_CHANNEL_LIQUIDITY: &str = "/v1/estimateChannelLiquidity";
    /// Websocket
    pub const WEBSOCKET: &str = "/v1/ws";

    /// List on chain and channel funds
    pub const LIST_FUNDS: &str = "/v1/listFunds";

    /// --- Peers ---
    /// Connect with a network peer.
    pub const CONNECT_PEER: &str = "/v1/peer/connect";
    /// Returns the list of peers connected with the node.
    pub const LIST_PEERS: &str = "/v1/peer/listPeers";
    /// Disconnect from a connected network peer.
    pub const DISCONNECT_PEER: &str = "/v1/peer/disconnect/:id";

    /// --- Channels ---
    /// Get the list of channels open on the node.
    pub const LIST_CHANNELS: &str = "/v1/channel/listChannels";
    /// Get the list of channels for this nodes peers.
    pub const LIST_PEER_CHANNELS: &str = "/v1/channel/listPeerChannels";
    /// Open channel with a connected peer node.
    pub const OPEN_CHANNEL: &str = "/v1/channel/openChannel";
    /// Update channel fee policy.
    pub const SET_CHANNEL_FEE: &str = "/v1/channel/setChannelFee";
    /// Close an existing channel with a peer.
    pub const CLOSE_CHANNEL: &str = "/v1/channel/closeChannel/:id";
    /// Fetch aggregate channel local and remote balances.
    pub const LOCAL_REMOTE_BALANCE: &str = "/v1/channel/localremotebal";
    /// Fetch the list of the forwarded htlcs.
    pub const LIST_FORWARDS: &str = "/v1/channel/listForwards";
    /// Fetch our channel history.
    pub const LIST_CHANNEL_HISTORY: &str = "/v1/channel/history";

    /// --- Network ---
    /// Look up a node on the network.
    pub const LIST_NETWORK_NODE: &str = "/v1/network/listNode/:id";
    /// Return list of all nodes on the network
    pub const LIST_NETWORK_NODES: &str = "/v1/network/listNode";
    /// Look up a channel on the network
    pub const LIST_NETWORK_CHANNEL: &str = "/v1/network/listChannel/:id";
    /// Return list of all channels on the network
    pub const LIST_NETWORK_CHANNELS: &str = "/v1/network/listChannel";
    /// Return feerate estimates, either satoshi-per-kw or satoshi-per-kb
    pub const FEE_RATES: &str = "/v1/network/feeRates/:style";

    /// --- On chain wallet ---
    /// Returns total, confirmed and unconfirmed on-chain balances.
    pub const GET_BALANCE: &str = "/v1/getBalance";
    /// Generate address for recieving on-chain funds.
    pub const NEW_ADDR: &str = "/v1/newaddr";
    /// Withdraw on-chain funds to an address.
    pub const WITHDRAW: &str = "/v1/withdraw";

    /// --- Payments ---
    /// Send funds to a node without an invoice.
    pub const KEYSEND: &str = "/v1/pay/keysend";
    /// Pay a  bolt11 invoice.
    pub const PAY_INVOICE: &str = "/v1/pay";
    /// List payments.
    pub const LIST_PAYMENTS: &str = "/v1/pay/listPayments";

    /// --- Invoices ---
    /// Generate a bolt11 invoice.
    pub const GENERATE_INVOICE: &str = "/v1/invoice/genInvoice";
    /// List the invoices on the node
    pub const LIST_INVOICES: &str = "/v1/invoice/listInvoices";
}

#[derive(Serialize, Deserialize)]
pub struct Error {
    pub status: String,
    pub detail: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    pub address: Vec<String>,
    pub fees_collected_msat: u64,
}

#[derive(Serialize, Deserialize)]
pub struct Chain {
    pub chain: String,
    pub network: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkChannel {
    pub source: String,
    pub destination: String,
    pub short_channel_id: u64,
    pub public: bool,
    pub satoshis: u64,
    pub amount_msat: u64,
    pub channel_flags: u8,
    pub active: bool,
    pub last_update: u32,
    pub base_fee_millisatoshi: u32,
    pub fee_per_millionth: u32,
    pub delay: u16,
    pub htlc_minimum_msat: u64,
    pub htlc_maximum_msat: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletBalance {
    pub total_balance: u64,
    pub conf_balance: u64,
    pub unconf_balance: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletTransfer {
    /// Any Bitcoin accepted type, including bech32
    pub address: String,
    /// Amount to be withdrawn. The string "all" can be used to specify withdrawal of all available funds
    pub satoshis: String,
    /// urgent, normal or slow
    pub fee_rate: Option<FeeRate>,
    /// minimum number of confirmations that used outputs should have
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

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum OutputStatus {
    Unconfirmed,
    Confirmed,
}

#[derive(Serialize, Deserialize)]
pub struct ListFundsOutput {
    pub txid: String,
    pub output: u32,
    pub amount_msat: u64,
    pub address: String,
    pub scriptpubkey: String,
    pub status: OutputStatus,
    #[serde(rename = "blockheight")]
    pub block_height: Option<u32>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFundsChannel {
    pub peer_id: String,
    pub connected: bool,
    pub state: ChannelState,
    pub short_channel_id: String,
    pub channel_sat: u64,
    pub our_amount_msat: u64,
    pub amount_msat: u64,
    pub funding_txid: String,
    pub funding_output: u16,
}

#[derive(Serialize, Deserialize)]
pub struct ListFunds {
    pub outputs: Vec<ListFundsOutput>,
    pub channels: Vec<ListFundsChannel>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum ChannelState {
    Usable,
    Ready,
    Pending,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    /// Pub key
    pub id: String,
    /// Peer connection status (true or false)
    pub connected: bool,
    // Channel connection status
    pub state: ChannelState,
    /// Channel ID
    pub short_channel_id: String,
    /// Channel ID
    pub channel_id: String,
    /// Channel funding transaction
    pub funding_txid: String,
    /// Private channel flag (true or false)
    pub private: bool,
    /// Number of msats on our side
    pub msatoshi_to_us: u64,
    /// Total msats in the channel
    pub msatoshi_total: u64,
    /// Number of msats to push to their side
    pub msatoshi_to_them: u64,
    /// Minimum number of msats on their side
    pub their_channel_reserve_satoshis: u64,
    /// Minimum number of msats on our side
    pub our_channel_reserve_satoshis: Option<u64>,
    /// Spendable msats
    pub spendable_msatoshi: u64,
    ///
    /// pub funding_allocation_msat: String,
    /// Flag indicating if this peer initiated the channel (0,1)
    pub direction: u8,
    /// Alias of the node
    pub alias: String,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FundChannel {
    /// Pub key of the peer
    pub id: String,
    /// Amount in satoshis
    pub satoshis: String,
    /// urgent/normal/slow/<sats>perkw/<sats>perkb
    pub fee_rate: Option<FeeRate>,
    /// Flag to announce the channel
    pub announce: Option<bool>,
    /// Minimum number of confirmations that used outputs should have
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

#[derive(Clone, Debug, PartialEq, Default)]
pub enum FeeRate {
    Urgent,
    #[default]
    Normal,
    Slow,
    PerKw(u32),
    PerKb(u32),
}

impl Serialize for FeeRate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            FeeRate::Urgent => serializer.serialize_str("urgent"),
            FeeRate::Normal => serializer.serialize_str("normal"),
            FeeRate::Slow => serializer.serialize_str("slow"),
            FeeRate::PerKw(x) => serializer.serialize_str(&format!("{x}perkw")),
            FeeRate::PerKb(x) => serializer.serialize_str(&format!("{x}perkb")),
        }
    }
}

struct FeeRateVisitor;

impl<'de> Visitor<'de> for FeeRateVisitor {
    type Value = FeeRate;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("urgent/normal/slow/<sats>perkw/<sats>perkb")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        v.parse()
            .map_err(|e: ParseFeeRateError| serde::de::Error::custom(e.0))
    }
}

impl<'de> Deserialize<'de> for FeeRate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(FeeRateVisitor)
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct ParseFeeRateError(String);
impl Display for ParseFeeRateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ParseFeeRateError: {}", self.0)
    }
}

impl std::error::Error for ParseFeeRateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl FromStr for FeeRate {
    type Err = ParseFeeRateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "urgent" => Ok(FeeRate::Urgent),
            "normal" => Ok(FeeRate::Normal),
            "slow" => Ok(FeeRate::Slow),
            _ => {
                if s.ends_with("perkw") {
                    Ok(FeeRate::PerKw(
                        s.trim_end_matches("perkw")
                            .parse::<u32>()
                            .map_err(|_| ParseFeeRateError("expected u32 for perkw".to_string()))?,
                    ))
                } else if s.ends_with("perkb") {
                    Ok(FeeRate::PerKb(
                        s.trim_end_matches("perkb")
                            .parse::<u32>()
                            .map_err(|_| ParseFeeRateError("expected u32 for perkb".to_string()))?,
                    ))
                } else {
                    Err(ParseFeeRateError("unknown fee rate. Expecting one of urgent/normal/slow/<sats>perkw/<sats>perkb".to_string()))
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeRates {
    pub urgent: u32,
    pub normal: u32,
    pub slow: u32,
    pub min_acceptable: u32,
    pub max_acceptable: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnChainFeeEstimates {
    pub opening_channel_satoshis: u32,
    pub mutual_close_satoshis: u32,
    pub unilateral_close_satoshis: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeRatesResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub perkb: Option<FeeRates>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub perkw: Option<FeeRates>,
    pub onchain_fee_estimates: OnChainFeeEstimates,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundChannelResponse {
    /// Transaction
    pub tx: Transaction,
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
#[serde(rename_all = "camelCase")]
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

#[derive(Serialize, Deserialize, PartialEq)]
pub struct Peer {
    pub id: String,
    pub connected: bool,
    pub netaddr: Option<String>,
    pub alias: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkNode {
    #[serde(rename = "nodeid")]
    pub node_id: String,
    pub alias: String,
    pub color: String,
    pub last_timestamp: u32,
    pub features: String,
    pub addresses: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct SignRequest {
    pub message: String,
}

#[derive(Serialize, Deserialize)]
pub struct SignResponse {
    pub signature: String,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KeysendRequest {
    // 33 byte, hex-encoded, pubkey of the node
    pub pubkey: String,
    // Amount in milli satoshis
    pub amount: u64,
    // Label for the payment
    pub label: Option<String>,
    // Fraction of the amount to be paid as fee (as a percentage)
    pub maxfeepercent: Option<f64>,
    // Keep retryinig to find routes for this long (seconds)
    pub retry_for: Option<u64>,
    // The payment can be delayed for more than this many blocks
    pub maxdelay: Option<u64>,
    // Amount for which the maxfeepercent check is skipped
    pub exemptfee: Option<u64>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentResponse {
    pub destination: String,
    pub payment_hash: String,
    pub created_at: u64,
    pub parts: u64,
    pub amount_msat: Option<u64>,
    pub amount_sent_msat: u64,
    pub payment_preimage: String,
    pub status: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GenerateInvoice {
    // Amount in milli satoshis
    pub amount: u64,
    // Unique label for the invoice
    pub label: String,
    // Description for the invoice
    pub description: String,
    // Expiry time period for the invoice (seconds)
    pub expiry: Option<u32>,
    // Include routing hints for private channels (true or 1)
    pub private: Option<bool>,
    //  The fallbacks array is one or more fallback addresses to include in the invoice (in order from most-preferred to least).
    pub fallbacks: Option<Vec<String>>,
    // 64-digit hex string to be used as payment preimage for the created invoice. IMPORTANT> if you specify the preimage, you are responsible, to ensure appropriate care for generating using a secure pseudorandom generator seeded with sufficient entropy, and keeping the preimage secret. This parameter is an advanced feature intended for use with cutting-edge cryptographic protocols and should not be used unless explicitly needed.
    pub preimage: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum InvoiceStatus {
    Unpaid,
    Paid,
    Expired,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Invoice {
    pub label: Option<String>,
    pub bolt11: String,
    pub payment_hash: String,
    pub description: String,
    pub status: InvoiceStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_msat: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_received_msat: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paid_at: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateInvoiceResponse {
    pub payment_hash: String,
    pub expires_at: u32,
    pub bolt11: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayInvoice {
    pub bolt11: String,
    pub label: Option<String>,
}

#[test]
fn test_fee_rate() -> Result<(), ParseFeeRateError> {
    let urgent_fee_rate = FeeRate::from_str("urgent")?;
    assert_eq!(urgent_fee_rate, FeeRate::Urgent);

    let normal_fee_rate = FeeRate::from_str("normal")?;
    assert_eq!(normal_fee_rate, FeeRate::Normal);

    let slow_fee_rate = FeeRate::from_str("slow")?;
    assert_eq!(slow_fee_rate, FeeRate::Slow);

    let pkb_fee_rate = FeeRate::from_str("50perkb")?;
    assert_eq!(pkb_fee_rate, FeeRate::PerKb(50));

    let pkw_fee_rate = FeeRate::from_str("37perkw")?;
    assert_eq!(pkw_fee_rate, FeeRate::PerKw(37));
    Ok(())
}
