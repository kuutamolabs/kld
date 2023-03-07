use anyhow::anyhow;
use api::NewAddress;
use api::NewAddressResponse;
use api::WalletBalance;
use api::WalletTransfer;
use api::WalletTransferResponse;
use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use bitcoin::Address;
use log::{info, warn};
use std::str::FromStr;
use std::sync::Arc;

use crate::handle_bad_request;
use crate::handle_err;
use crate::handle_unauthorized;

use super::KldMacaroon;
use super::MacaroonAuth;
use super::WalletInterface;

pub(crate) async fn get_balance(
    macaroon: KldMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(wallet): Extension<Arc<dyn WalletInterface + Send + Sync>>,
) -> Result<impl IntoResponse, StatusCode> {
    handle_unauthorized!(macaroon_auth.verify_readonly_macaroon(&macaroon.0));

    let balance = handle_err!(wallet.balance());
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
) -> Result<impl IntoResponse, StatusCode> {
    handle_unauthorized!(macaroon_auth.verify_admin_macaroon(&macaroon.0));

    if let Some(address_type) = new_address.address_type {
        if address_type != "bech32" {
            handle_bad_request!(Err(anyhow!("Unsupported address type")))
        }
    }
    let address_info = handle_err!(wallet.new_address());
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
) -> Result<impl IntoResponse, StatusCode> {
    handle_unauthorized!(macaroon_auth.verify_admin_macaroon(&macaroon.0));

    let address = handle_bad_request!(Address::from_str(&wallet_transfer.address));
    let amount = if wallet_transfer.satoshis == "all" {
        u64::MAX
    } else {
        handle_bad_request!(u64::from_str(&wallet_transfer.satoshis))
    };
    let tx = handle_err!(wallet.transfer(address, amount, None, None, vec![]).await);
    let tx_str = handle_err!(serde_json::to_string(&tx));
    let response = WalletTransferResponse {
        tx: tx_str,
        txid: tx.txid().to_string(),
    };
    Ok(Json(response))
}
