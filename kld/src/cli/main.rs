mod client;

use crate::client::Api;
use anyhow::Result;
use api::FeeRate;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// IP address or hostname of the target machine.
    #[arg(short, long)]
    target: String,
    /// Path to the TLS cert of the target API.
    #[arg(short, long)]
    cert_path: String,
    /// Path to the macaroon for authenticating with the API.
    #[arg(short, long)]
    macaroon_path: String,
    /// Command to run.
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Fetch information about this lightning node.
    GetInfo,
    /// Creates a signature of the message using node's secret key (message limit 65536 chars)
    Sign {
        /// Message to be signed (max 65536 chars)
        #[arg(long)]
        message: String,
    },
    /// Fetch confirmed and unconfirmed on-chain balance.
    GetBalance,
    /// Generates new on-chain address for receiving funds.
    NewAddress,
    /// Send on-chain funds out of the wallet.
    Withdraw {
        /// The address to withdraw to.
        #[arg(long)]
        address: String,
        /// The amount to withdraw (in Satoshis). The string "all" will empty the wallet.
        #[arg(long)]
        satoshis: String,
        /// Fee rate [urgent/normal/slow/<sats>perkw/<sats>perkb]
        #[arg(long)]
        fee_rate: Option<FeeRate>,
    },
    /// Show available funds from the internal wallet.
    ListFunds,
    /// Fetch a list of this nodes peers.
    ListPeers,
    /// Connect with a network peer.
    ConnectPeer {
        /// The public key (id) of the node to connect to. Optionally provide host and port [id@host:port].
        #[arg(long)]
        public_key: String,
    },
    /// Disconnect from a network peer.
    DisconnectPeer {
        /// The public key of the node to disconnect from.
        #[arg(long)]
        public_key: String,
    },
    /// Fetch a list of this nodes open channels.
    ListChannels,
    /// Open a channel with another node.
    OpenChannel {
        /// The public key of the node to open a channel with. Optionally provide host and port [id@host:port].
        #[arg(long)]
        public_key: String,
        /// Amount of satoshis to commit to the channel.
        #[arg(long)]
        sats: String,
        /// The number of satoshis to push to the other node side of the channel.
        #[arg(long)]
        push_msat: Option<String>,
        /// Whether to announce the channel to the rest of the network (public - default) or not (private).
        #[arg(long)]
        announce: Option<bool>,
        /// Fee rate [urgent/normal/slow/<sats>perkw/<sats>perkb]
        #[arg(long)]
        fee_rate: Option<FeeRate>,
    },
    /// Set channel fees.
    SetChannelFee {
        /// Channel ID, short channel ID or "all" for all channels.
        #[arg(long)]
        id: String,
        /// Optional value in msats added as base fee to any routed payment.
        #[arg(long)]
        base_fee: Option<u32>,
        /// Optional value that is added proportionally per-millionths to any routed payment volume in satoshi
        #[arg(long)]
        ppm_fee: Option<u32>,
    },
    /// Close a channel.
    CloseChannel {
        /// Channel ID or short channel ID to close.
        #[arg(long)]
        id: String,
    },
    /// Get node information from the network graph.
    NetworkNodes {
        /// Provide Node ID to get info about a single node.
        #[arg(long)]
        id: Option<String>,
    },
    /// Get channel information from the network graph.
    NetworkChannels {
        /// Provide short channel ID to get info about a single channel.
        #[arg(long)]
        id: Option<String>,
    },
    /// Return feerate estimates, either satoshi-per-kw or satoshi-per-kb.
    FeeRates {
        /// perkb (default) or perkw
        #[arg(long)]
        style: Option<String>,
    },
    /// Pay a node without an invoice.
    Keysend {
        /// Node ID of the payee.
        #[arg(long)]
        public_key: String,
        /// Amount to pay in sats.
        #[arg(long)]
        amount: u64,
    },
    /// Generate a bolt11 invoice for receiving a payment.
    GenerateInvoice {
        // Amount in milli satoshis
        #[arg(long)]
        amount: u64,
        // Unique label for the invoice
        #[arg(long)]
        label: String,
        // Description for the invoice
        #[arg(long)]
        description: String,
        // Expiry time period for the invoice (seconds)
        #[arg(long)]
        expiry: Option<u32>,
    },
    /// List all invoices
    ListInvoices {
        // Label of the invoice
        #[arg(long)]
        label: Option<String>,
    },
    /// Pay an invoice
    PayInvoice {
        // The invoice to pay
        #[arg(long)]
        bolt11: String,
        // Label for the payment
        #[arg(long)]
        label: Option<String>,
    },
}

fn main() {
    let args = Args::parse();

    if let Err(e) = run_command(args) {
        eprintln!("Error executing command: {e}");
        std::process::exit(1);
    }
}

fn run_command(args: Args) -> Result<()> {
    let api = Api::new(&args.target, &args.cert_path, &args.macaroon_path)?;

    let output = match args.command {
        Command::Sign { message } => api.sign(message)?,
        Command::GetInfo => api.get_info()?,
        Command::GetBalance => api.get_balance()?,
        Command::NewAddress => api.new_address()?,
        Command::Withdraw {
            address,
            satoshis,
            fee_rate,
        } => api.withdraw(address, satoshis, fee_rate)?,
        Command::ListFunds => api.list_funds()?,
        Command::ListChannels => api.list_channels()?,
        Command::ListPeers => api.list_peers()?,
        Command::ConnectPeer { public_key } => api.connect_peer(public_key)?,
        Command::DisconnectPeer { public_key } => api.disconnect_peer(public_key)?,
        Command::OpenChannel {
            public_key,
            sats: satoshis,
            push_msat,
            announce,
            fee_rate,
        } => api.open_channel(public_key, satoshis, push_msat, announce, fee_rate)?,
        Command::SetChannelFee {
            id,
            base_fee,
            ppm_fee,
        } => api.set_channel_fee(id, base_fee, ppm_fee)?,
        Command::CloseChannel { id } => api.close_channel(id)?,
        Command::NetworkNodes { id } => api.list_network_nodes(id)?,
        Command::NetworkChannels { id } => api.list_network_channels(id)?,
        Command::FeeRates { style } => api.fee_rates(style)?,
        Command::Keysend { public_key, amount } => api.keysend(public_key, amount)?,
        Command::GenerateInvoice {
            amount,
            label,
            description,
            expiry,
        } => api.generate_invoice(amount, label, description, expiry)?,
        Command::ListInvoices { label } => api.list_invoices(label)?,
        Command::PayInvoice { bolt11, label } => api.pay_invoice(bolt11, label)?,
    };
    if output != "null" {
        println!("{output}");
    }
    Ok(())
}
