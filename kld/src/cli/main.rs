mod client;
mod commands;

use crate::client::Api;
use anyhow::{bail, Result};
use clap::Parser;
use commands::{KldCliCommand, KldCliSubCommand};

fn main() {
    let args = KldCliCommand::parse();

    if let Err(e) = run_command(args) {
        eprintln!("Error executing command: {e}");
        std::process::exit(1);
    }
}

fn run_command(args: KldCliCommand) -> Result<()> {
    let api = Api::new(args.target, args.cert_path, args.macaroon_path)?;

    let output = match args.command {
        KldCliSubCommand::Sign { message } => api.sign(message)?,
        KldCliSubCommand::GetInfo => api.get_info()?,
        KldCliSubCommand::GetBalance => api.get_balance()?,
        KldCliSubCommand::NewAddress => api.new_address()?,
        KldCliSubCommand::Withdraw {
            address,
            amount: satoshis,
            fee_rate,
        } => api.withdraw(address, satoshis, fee_rate)?,
        KldCliSubCommand::ListFunds => api.list_funds()?,
        KldCliSubCommand::ListActiveChannels => api.list_active_channels()?,
        KldCliSubCommand::ListPeers => api.list_peers()?,
        KldCliSubCommand::ConnectPeer { public_key } => api.connect_peer(public_key)?,
        KldCliSubCommand::DisconnectPeer { public_key } => api.disconnect_peer(public_key)?,
        KldCliSubCommand::OpenChannel {
            public_key,
            sats: satoshis,
            push_msat,
            announce,
            fee_rate,
        } => api.open_channel(public_key, satoshis, push_msat, announce, fee_rate)?,
        KldCliSubCommand::SetChannelFee {
            id,
            base_fee,
            ppm_fee,
        } => api.set_channel_fee(id, base_fee, ppm_fee)?,
        KldCliSubCommand::CloseChannel {
            id,
            force_close: None,
        } => api.close_channel(id)?,
        KldCliSubCommand::CloseChannel {
            id,
            force_close: Some(broadcast_flag),
        } => {
            let need_broadcast = match broadcast_flag.as_str() {
                "broadcast" => true,
                "no-broadcast" => false,
                _ => bail!("the broadcast-flag need to `broadcast` or `no-broadcast`"),
            };
            api.force_close_channel(id, need_broadcast)?
        }
        KldCliSubCommand::NetworkNodes { id } => api.list_network_nodes(id)?,
        KldCliSubCommand::NetworkChannels { id } => api.list_network_channels(id)?,
        KldCliSubCommand::FeeRates { style } => api.fee_rates(style)?,
        KldCliSubCommand::Keysend { public_key, amount } => api.keysend(public_key, amount)?,
        KldCliSubCommand::GenerateInvoice {
            amount,
            label,
            description,
            expiry,
        } => api.generate_invoice(amount, label, description, expiry)?,
        KldCliSubCommand::ListInvoices { label } => api.list_invoices(label)?,
        KldCliSubCommand::PayInvoice { bolt11, label } => api.pay_invoice(bolt11, label)?,
        KldCliSubCommand::ListPayments { bolt11, direction } => {
            api.list_payments(bolt11, direction)?
        }
        KldCliSubCommand::EstimateChannelLiquidity { scid, target } => {
            api.estimate_channel_liquidity(scid, target)?
        }
        KldCliSubCommand::LocalRemoteBalance => api.local_remote_balance()?,
        KldCliSubCommand::GetFees => api.get_fees()?,
        KldCliSubCommand::ListForwards { status } => api.list_forwards(status)?,
        KldCliSubCommand::ListClosedChannels => api.list_closed_channels()?,
        KldCliSubCommand::Decode { invoice } => api.decode(invoice)?,
        KldCliSubCommand::Scorer { path } => api.scorer(path.unwrap_or("scorer.bin".into()))?,
    };
    if output != "null" {
        println!("{output}");
    }
    Ok(())
}
