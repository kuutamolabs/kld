mod client;

use crate::client::Api;
use anyhow::Result;
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
    },
    /// Fetch a list of this nodes peers.
    ListPeers,
    /// Connect with a network peer.
    ConnectPeer {
        /// The public key of the node to connect to.
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
        /// The public key of the node to open a channel with.
        #[arg(long)]
        public_key: String,
        /// Amount of satoshis to commit to the channel.
        #[arg(long)]
        satoshis: String,
        /// The number of satoshis to push to the other node side of the channel.
        #[arg(long)]
        push_msat: Option<String>,
    },
}

fn main() {
    let args = Args::parse();

    match run_command(args) {
        Ok(_) => (),
        Err(e) => println!("Error executing command: {}", e),
    }
}

fn run_command(args: Args) -> Result<()> {
    let api = Api::new(&args.target, &args.cert_path, &args.macaroon_path)?;

    let result = match args.command {
        Command::GetInfo => api.get_info()?,
        Command::GetBalance => api.get_balance()?,
        Command::NewAddress => api.new_address()?,
        Command::Withdraw { address, satoshis } => api.withdraw(address, satoshis)?,
        Command::ListChannels => api.list_channels()?,
        Command::ListPeers => api.list_peers()?,
        Command::ConnectPeer { public_key } => api.connect_peer(public_key)?,
        Command::DisconnectPeer { public_key } => api.disconnect_peer(public_key)?,
        Command::OpenChannel {
            public_key,
            satoshis,
            push_msat,
        } => api.open_channel(public_key, satoshis, push_msat)?,
    };
    println!("{}", result);
    Ok(())
}
