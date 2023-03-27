use anyhow::anyhow;
use api::NewAddress;
use api::NewAddressResponse;
use api::WalletBalance;
use api::WalletTransfer;
use api::WalletTransferResponse;
use axum::{response::IntoResponse, Extension, Json};
use bitcoin::consensus::encode;
use bitcoin::Address;
use std::str::FromStr;
use std::sync::Arc;

use crate::wallet::WalletInterface;

use super::bad_request;
use super::internal_server;
use super::unauthorized;
use super::ApiError;
use super::KldMacaroon;
use super::MacaroonAuth;

pub(crate) async fn get_balance(
    macaroon: KldMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(wallet): Extension<Arc<dyn WalletInterface + Send + Sync>>,
) -> Result<impl IntoResponse, ApiError> {
    macaroon_auth
        .verify_readonly_macaroon(&macaroon.0)
        .map_err(unauthorized)?;

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
    macaroon: KldMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(wallet): Extension<Arc<dyn WalletInterface + Send + Sync>>,
    Json(new_address): Json<NewAddress>,
) -> Result<impl IntoResponse, ApiError> {
    macaroon_auth
        .verify_admin_macaroon(&macaroon.0)
        .map_err(unauthorized)?;

    if let Some(address_type) = new_address.address_type {
        if address_type != "bech32" {
            return Err(bad_request(anyhow!("Unsupported address type")));
        }
    }
    let address_info = wallet.new_address().map_err(internal_server)?;
    let response = NewAddressResponse {
        address: address_info.address.to_string(),
    };
    Ok(Json(response))
}

pub(crate) async fn transfer(
    macaroon: KldMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(wallet): Extension<Arc<dyn WalletInterface + Send + Sync>>,
    Json(wallet_transfer): Json<WalletTransfer>,
) -> Result<impl IntoResponse, ApiError> {
    macaroon_auth
        .verify_admin_macaroon(&macaroon.0)
        .map_err(unauthorized)?;

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
