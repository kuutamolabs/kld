use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct KldCliCommand {
    /// IP address or hostname of the target machine.
    #[arg(short, long, hide = true)]
    pub target: String,
    /// Path to the TLS cert of the target API.
    #[arg(short, long, hide = true)]
    pub cert_path: String,
    /// Path to the macaroon for authenticating with the API.
    #[arg(short, long, hide = true)]
    pub macaroon_path: String,
    /// Command to run.
    #[clap(subcommand)]
    pub command: KldCliSubCommand,
}

#[derive(Subcommand, Debug)]
pub enum KldCliSubCommand {
    /// Fetch information about this lightning node.
    GetInfo,
    /// Creates a signature of the message using node's secret key (message limit 65536 chars)
    Sign {
        /// Message to be signed (max 65536 chars)
        #[arg()]
        message: String,
    },
    /// Fetch confirmed and unconfirmed on-chain balance.
    GetBalance,
    /// Generates new on-chain address for receiving funds.
    NewAddress,
    /// Send on-chain funds out of the wallet.
    Withdraw {
        /// The address to withdraw to.
        #[arg()]
        address: String,
        /// The amount to withdraw (in Satoshis). The string "all" will empty the wallet.
        #[arg()]
        satoshis: String,
        /// Fee rate [urgent/normal/slow/<sats>perkw/<sats>perkb]
        #[arg(short, long)]
        fee_rate: Option<String>,
    },
    /// Show available funds from the internal wallet.
    ListFunds,
    /// Fetch a list of this nodes peers.
    ListPeers,
    /// Connect with a network peer.
    ConnectPeer {
        /// The public key (id) of the node to connect to. Optionally provide host and port [id@host:port].
        #[arg()]
        public_key: String,
    },
    /// Disconnect from a network peer.
    DisconnectPeer {
        /// The public key of the node to disconnect from.
        #[arg()]
        public_key: String,
    },
    /// Fetch a list of this nodes open channels.
    ListPeerChannels,
    /// Open a channel with another node.
    OpenChannel {
        /// The public key of the node to open a channel with. Optionally provide host and port [id@host:port].
        #[arg()]
        public_key: String,
        /// Amount of satoshis to commit to the channel.
        #[arg()]
        sats: String,
        /// The number of satoshis to push to the other node side of the channel.
        #[arg(short, long)]
        push_msat: Option<String>,
        /// Whether to announce the channel to the rest of the network (public - default) or not (private).
        #[arg(short, long)]
        announce: Option<bool>,
        /// Fee rate [urgent/normal/slow/<sats>perkw/<sats>perkb]
        #[arg(short, long)]
        fee_rate: Option<String>,
    },
    /// Set channel fees.
    SetChannelFee {
        /// Channel ID, short channel ID or "all" for all channels.
        #[arg()]
        id: String,
        /// Optional value in msats added as base fee to any routed payment.
        #[arg(short, long)]
        base_fee: Option<u32>,
        /// Optional value that is added proportionally per-millionths to any routed payment volume in satoshi
        #[arg(short, long)]
        ppm_fee: Option<u32>,
    },
    /// Close a channel.
    CloseChannel {
        /// Channel ID or short channel ID to close.
        #[arg()]
        id: String,
    },
    /// Get node information from the network graph.
    NetworkNodes {
        /// Provide Node ID to get info about a single node.
        #[arg(short, long)]
        id: Option<String>,
    },
    /// Get channel information from the network graph.
    NetworkChannels {
        /// Provide short channel ID to get info about a single channel.
        #[arg(short, long)]
        id: Option<String>,
    },
    /// Return feerate estimates, either satoshi-per-kw or satoshi-per-kb.
    FeeRates {
        /// perkb (default) or perkw
        #[arg(short, long)]
        style: Option<String>,
    },
    /// Pay a node without an invoice.
    Keysend {
        /// Node ID of the payee.
        #[arg()]
        public_key: String,
        /// Amount to pay in sats.
        #[arg()]
        amount: u64,
    },
    /// Generate a bolt11 invoice for receiving a payment.
    GenerateInvoice {
        /// Amount in milli satoshis
        #[arg()]
        amount: u64,
        /// Unique label for the invoice
        #[arg()]
        label: String,
        /// Description for the invoice
        #[arg()]
        description: String,
        /// Expiry time period for the invoice (seconds)
        #[arg(short, long)]
        expiry: Option<u32>,
    },
    /// List all invoices
    ListInvoices {
        /// Label of the invoice
        #[arg(short, long)]
        label: Option<String>,
    },
    /// Pay an invoice
    PayInvoice {
        /// The invoice to pay
        #[arg()]
        bolt11: String,
        /// Label for the payment
        #[arg(short, long)]
        label: Option<String>,
    },
    /// List all payments
    ListPayments {
        /// Bolt11 invoice of payment
        #[arg(short, long)]
        bolt11: Option<String>,
        /// Direction (inbound/outbound)
        #[arg(short, long)]
        direction: Option<String>,
    },
    /// Esimate channel liquidity to a target node
    EstimateChannelLiquidity {
        /// Short channel ID
        #[arg()]
        scid: u64,
        /// Bolt11 invoice of payment
        #[arg()]
        target: String,
    },
    /// Fetch the aggregate local and remote channel balances (msat) of the node
    LocalRemoteBalance,
    /// Get node routing fees.
    GetFees,
    /// Fetch a list of the forwarded htlcs.
    ListForwards {
        /// The status of the forwards (succeeded, failed)
        #[arg(short, long)]
        status: Option<String>,
    },
    /// Fetch a list of historic (closed) channels
    ListChannelHistory,
}
