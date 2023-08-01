use anyhow::anyhow;
use api::ChannelState;
use api::ListFunds;
use api::ListFundsChannel;
use api::ListFundsOutput;
use api::NewAddress;
use api::NewAddressResponse;
use api::OutputStatus;
use api::WalletBalance;
use api::WalletTransfer;
use api::WalletTransferResponse;
use axum::{response::IntoResponse, Extension, Json};
use bitcoin::consensus::encode;
use bitcoin::Address;
use std::str::FromStr;
use std::sync::Arc;

use crate::ldk::LightningInterface;
use crate::ldk::PeerStatus;
use crate::to_string_empty;
use crate::wallet::WalletInterface;

use super::{bad_request, internal_server, ApiError};

pub(crate) async fn get_balance(
    Extension(wallet): Extension<Arc<dyn WalletInterface + Send + Sync>>,
) -> Result<impl IntoResponse, ApiError> {
    let balance = wallet.balance().map_err(internal_server)?;
    let unconf_balance = balance.untrusted_pending + balance.trusted_pending;
    let total_balance = unconf_balance + balance.confirmed;
    let result = WalletBalance {
        total_balance,
        conf_balance: balance.confirmed,
        unconf_balance,
    };
    Ok(Json(result))
}

pub(crate) async fn new_address(
    Extension(wallet): Extension<Arc<dyn WalletInterface + Send + Sync>>,
    Json(new_address): Json<NewAddress>,
) -> Result<impl IntoResponse, ApiError> {
    if let Some(address_type) = new_address.address_type {
        if address_type != "bech32" {
            return Err(bad_request(anyhow!("Unsupported address type")));
        }
    }
    let address_info = wallet.new_external_address().map_err(internal_server)?;
    let response = NewAddressResponse {
        address: address_info.address.to_string(),
    };
    Ok(Json(response))
}

pub(crate) async fn transfer(
    Extension(wallet): Extension<Arc<dyn WalletInterface + Send + Sync>>,
    Json(wallet_transfer): Json<WalletTransfer>,
) -> Result<impl IntoResponse, ApiError> {
    let address = Address::from_str(&wallet_transfer.address).map_err(bad_request)?;
    let amount = if wallet_transfer.satoshis == "all" {
        u64::MAX
    } else {
        u64::from_str(&wallet_transfer.satoshis).map_err(bad_request)?
    };
    let (tx, tx_details) = wallet
        .transfer(address, amount, wallet_transfer.fee_rate, None, vec![])
        .await
        .map_err(internal_server)?;
    let tx_hex = encode::serialize_hex(&tx);
    let response = WalletTransferResponse {
        tx: tx_hex,
        txid: tx_details.txid.to_string(),
    };
    Ok(Json(response))
}

pub(crate) async fn list_funds(
    Extension(wallet): Extension<Arc<dyn WalletInterface + Send + Sync>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
) -> Result<impl IntoResponse, ApiError> {
    let mut outputs = vec![];
    let utxos = wallet.list_utxos().map_err(internal_server)?;
    for (utxo, detail) in utxos {
        outputs.push(ListFundsOutput {
            txid: utxo.outpoint.txid.to_string(),
            output: utxo.outpoint.vout,
            value: utxo.txout.value,
            amount_msat: utxo.txout.value * 1000,
            address: Address::from_script(&utxo.txout.script_pubkey, lightning_interface.network())
                .map(|a| a.to_string())
                .map_err(internal_server)?,
            status: if detail.confirmation_time.is_some() {
                OutputStatus::Confirmed
            } else {
                OutputStatus::Unconfirmed
            },
            block_height: detail.confirmation_time.map(|t| t.height),
        });
    }

    let mut channels = vec![];
    let peers = lightning_interface
        .list_peers()
        .await
        .map_err(internal_server)?;
    for channel in lightning_interface.list_channels() {
        if let Some(funding_txo) = channel.funding_txo {
            channels.push(ListFundsChannel {
                peer_id: channel.counterparty.node_id.to_string(),
                connected: peers
                    .iter()
                    .find(|p| p.public_key == channel.counterparty.node_id)
                    .map(|p| p.status == PeerStatus::Connected)
                    .unwrap_or_default(),
                state: if channel.is_usable {
                    ChannelState::Usable
                } else if channel.is_channel_ready {
                    ChannelState::Ready
                } else {
                    ChannelState::Pending
                },
                short_channel_id: to_string_empty!(channel.short_channel_id),
                our_amount_msat: channel.balance_msat,
                channel_sat: channel.channel_value_satoshis,
                amount_msat: channel.channel_value_satoshis * 1000,
                funding_txid: funding_txo.txid.to_string(),
                funding_output: funding_txo.index,
            });
        }
    }
    let response = ListFunds { outputs, channels };
    Ok(Json(response))
}
