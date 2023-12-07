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
/// Get the list of channels for this nodes peers.
pub const LIST_PEER_CHANNELS: &str = "/v1/channel/listPeerChannels";
/// Open channel with a connected peer node.
pub const OPEN_CHANNEL: &str = "/v1/channel/openChannel";
/// Update channel fee policy.
pub const SET_CHANNEL_FEE: &str = "/v1/channel/setChannelFee";
/// Close an existing channel with a peer.
pub const CLOSE_CHANNEL: &str = "/v1/channel/closeChannel/:id";
/// Force close an existing channel with a peer.
pub const FORCE_CLOSE_CHANNEL_WITH_BROADCAST: &str =
    "/v1/channel/forceCloseChannelWithBoradCast/:id";
pub const FORCE_CLOSE_CHANNEL_WITHOUT_BROADCAST: &str =
    "/v1/channel/forceCloseChannelWithoutBoradCast/:id";
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
/// Generate address for receiving on-chain funds.
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
/// Decode invoice
pub const DECODE_INVOICE: &str = "/v1/utility/decode/:invoice";

/// --- Scorer ---
pub const SCORER: &str = "/v1/scorer";
