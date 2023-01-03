use std::sync::Arc;

use api::Balance;
use axum::{http::StatusCode, response::IntoResponse, Extension, Json};

use super::KndMacaroon;
use super::MacaroonAuth;
use super::WalletInterface;

pub(crate) async fn get_balance(
    macaroon: KndMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(wallet): Extension<Arc<dyn WalletInterface + Send + Sync>>,
) -> Result<impl IntoResponse, StatusCode> {
    if macaroon_auth.verify_macaroon(&macaroon.0).is_err() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if let Ok(balance) = wallet.balance() {
        let unconf_balance = balance.untrusted_pending + balance.trusted_pending;
        let total_balance = unconf_balance + balance.confirmed;
        let result = Balance {
            total_balance,
            conf_balance: balance.confirmed,
            unconf_balance,
        };
        Ok(Json(result))
    } else {
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}
