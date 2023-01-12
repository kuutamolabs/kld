mod api;

use crate::api::Api;
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

    match args.command {
        Command::GetInfo => {
            let info = api.get_info()?;
            println!("{}", info);
        }
        Command::GetBalance => {
            let balance = api.get_balance()?;
            println!("{}", balance);
        }
        Command::ListChannels => {
            let channels = api.list_channels()?;
            println!("{}", channels);
        }
        Command::OpenChannel {
            public_key,
            satoshis,
            push_msat,
        } => {
            let result = api.open_channel(public_key, satoshis, push_msat)?;
            println!("{}", result);
        }
    }
    Ok(())
}
