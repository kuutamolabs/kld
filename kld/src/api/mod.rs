mod channels;
mod invoices;
mod macaroon_auth;
mod network;
pub mod payloads;
mod payments;
mod peers;
pub mod routes;
mod skt_addr;
mod utility;
mod wallet;
mod ws;

pub use skt_addr::SocketAddress;

pub use macaroon_auth::{KldMacaroon, MacaroonAuth};
use serde::{Deserialize, Deserializer};
use serde_json::json;

use self::utility::get_info;
use crate::{
    api::{
        channels::{
            channel_history, close_channel, close_channel_with_fee,
            force_close_channel_with_broadcast, force_close_channel_without_broadcast,
            list_channels, list_forwards, list_peer_channels, local_remote_balance, open_channel,
            set_channel_fee,
        },
        invoices::{decode_invoice, generate_invoice, list_invoices},
        macaroon_auth::{admin_auth, readonly_auth},
        network::{
            fee_rates, get_network_channel, get_network_node, list_network_channels,
            list_network_nodes,
        },
        payments::{keysend, list_payments, pay_invoice},
        peers::{connect_peer, disconnect_peer, list_peers},
        utility::{estimate_channel_liquidity_range, get_fees, score, sign},
        wallet::{get_balance, list_funds, new_address, transfer},
        ws::ws_handler,
    },
    bitcoind::bitcoind_interface::BitcoindInterface,
    ldk::LightningInterface,
    wallet::WalletInterface,
};
use anyhow::{Context, Result};
use axum::{
    extract::Extension,
    middleware::from_fn,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use axum_server::{
    tls_rustls::{RustlsAcceptor, RustlsConfig},
    Handle, Server,
};
use futures::{future::Shared, Future};
use hyper::StatusCode;
use log::{error, info, warn};
use std::{net::SocketAddr, str::FromStr, sync::Arc, time::Duration};
use tower_http::cors::CorsLayer;

pub const API_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct RestApi {
    server: Server<RustlsAcceptor>,
}

pub async fn bind_api_server(listen_address: String, certs_dir: String) -> Result<RestApi> {
    let rustls_config = config(&certs_dir)
        .await
        .context("failed to load tls configuration")?;
    let addr = listen_address.parse()?;
    info!("Starting REST API on {addr}");
    Ok(RestApi {
        server: axum_server::bind_rustls(addr, rustls_config),
    })
}

impl RestApi {
    pub async fn serve(
        self,
        bitcoind_api: Arc<dyn BitcoindInterface + Send + Sync>,
        lightning_api: Arc<dyn LightningInterface + Send + Sync>,
        wallet_api: Arc<dyn WalletInterface + Send + Sync>,
        macaroon_auth: Arc<MacaroonAuth>,
        quit_signal: Shared<impl Future<Output = ()>>,
    ) -> Result<()> {
        let cors = CorsLayer::permissive();
        let handle = Handle::new();
        let readonly_routes = Router::new()
            .route(routes::ROOT, get(root))
            .route(routes::GET_INFO, get(get_info))
            .route(
                routes::ESTIMATE_CHANNEL_LIQUIDITY,
                get(estimate_channel_liquidity_range),
            )
            .route(routes::GET_BALANCE, get(get_balance))
            .route(routes::LIST_FUNDS, get(list_funds))
            .route(routes::LIST_PEER_CHANNELS, get(list_peer_channels))
            .route(routes::LIST_PEERS, get(list_peers))
            .route(routes::LIST_NETWORK_NODE, get(get_network_node))
            .route(routes::LIST_NETWORK_NODES, get(list_network_nodes))
            .route(routes::LIST_NETWORK_CHANNEL, get(get_network_channel))
            .route(routes::LIST_NETWORK_CHANNELS, get(list_network_channels))
            .route(routes::FEE_RATES, get(fee_rates))
            .route(routes::LIST_INVOICES, get(list_invoices))
            .route(routes::LIST_PAYMENTS, get(list_payments))
            .route(routes::LOCAL_REMOTE_BALANCE, get(local_remote_balance))
            .route(routes::GET_FEES, get(get_fees))
            .route(routes::LIST_FORWARDS, get(list_forwards))
            .route(routes::LIST_CHANNEL_HISTORY, get(channel_history))
            .route(routes::LIST_CHANNELS, get(list_channels))
            .route(routes::DECODE_INVOICE, get(decode_invoice))
            .route(routes::SCORER, get(score))
            .layer(from_fn(readonly_auth));

        let admin_routes = Router::new()
            .route(routes::SIGN, post(sign))
            .route(routes::OPEN_CHANNEL, post(open_channel))
            .route(routes::SET_CHANNEL_FEE, post(set_channel_fee))
            .route(routes::CLOSE_CHANNEL, delete(close_channel))
            .route(
                routes::CLOSE_CHANNEL_WITH_FEE,
                delete(close_channel_with_fee),
            )
            .route(
                routes::FORCE_CLOSE_CHANNEL_WITH_BROADCAST,
                delete(force_close_channel_with_broadcast),
            )
            .route(
                routes::FORCE_CLOSE_CHANNEL_WITHOUT_BROADCAST,
                delete(force_close_channel_without_broadcast),
            )
            .route(routes::NEW_ADDR, get(new_address))
            .route(routes::WITHDRAW, post(transfer))
            .route(routes::CONNECT_PEER, post(connect_peer))
            .route(routes::DISCONNECT_PEER, delete(disconnect_peer))
            .route(routes::KEYSEND, post(keysend))
            .route(routes::GENERATE_INVOICE, post(generate_invoice))
            .route(routes::PAY_INVOICE, post(pay_invoice))
            .route(routes::WEBSOCKET, get(ws_handler))
            .layer(from_fn(admin_auth));

        let routes = readonly_routes
            .merge(admin_routes)
            .fallback(handler_404)
            .layer(cors)
            .layer(Extension(bitcoind_api))
            .layer(Extension(lightning_api))
            .layer(Extension(wallet_api))
            .layer(Extension(macaroon_auth));

        tokio::select!(
            result = self.server.serve(routes.into_make_service_with_connect_info::<SocketAddr>()) => {
                    if let Err(e) = result {
                        error!("API server shutdown unexpectedly: {}", e);
                    } else {
                        info!("API server shutdown successfully.");
                    }
                }
            _ = quit_signal => {
                handle.graceful_shutdown(Some(Duration::from_secs(30)));
            }
        );
        Ok(())
    }
}

async fn root() -> Result<impl IntoResponse, ApiError> {
    Ok(())
}

async fn handler_404() -> impl IntoResponse {
    ApiError::NotFound("No such method".to_string())
}

async fn config(certs_dir: &str) -> Result<RustlsConfig> {
    let cert = format!("{certs_dir}/kld.crt");
    let key = format!("{certs_dir}/kld.key");
    RustlsConfig::from_pem_file(&cert, &key)
        .await
        .with_context(|| format!("failed to load certificates ({cert}) and private key ({key})"))
}

pub enum ApiError {
    Unauthorized,
    NotFound(String),
    BadRequest(Box<dyn std::error::Error>),
    InternalServerError(Box<dyn std::error::Error>),
}

impl ApiError {
    fn bad_request_from_std_err(err: impl std::error::Error + 'static) -> Self {
        ApiError::BadRequest(Box::new(err))
    }

    fn internal_err_from_std_err(err: impl std::error::Error + 'static) -> Self {
        ApiError::InternalServerError(Box::new(err))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            ApiError::Unauthorized => build_api_error(
                StatusCode::UNAUTHORIZED,
                "Failed to verify macaroon".to_string(),
            ),
            ApiError::NotFound(s) => build_api_error(StatusCode::NOT_FOUND, s),
            ApiError::BadRequest(e) => build_api_error(StatusCode::BAD_REQUEST, e.to_string()),
            ApiError::InternalServerError(e) => {
                build_api_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
        }
    }
}

fn build_api_error(status_code: StatusCode, detail: String) -> Response {
    let error = crate::api::payloads::Error {
        status: status_code.to_string(),
        detail,
    };
    if let Ok(value) = serde_json::to_value(error) {
        (status_code, Json(value)).into_response()
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"status": StatusCode::INTERNAL_SERVER_ERROR.to_string()})),
        )
            .into_response()
    }
}

#[macro_export]
macro_rules! to_string_empty {
    ($v: expr) => {
        $v.map_or("".to_string(), |x| x.to_string())
    };
}

pub fn unauthorized(e: anyhow::Error) -> ApiError {
    info!("{}", e);
    ApiError::Unauthorized
}

pub fn internal_server(e: impl Into<anyhow::Error>) -> ApiError {
    let anyhow_err = e.into();
    warn!("{}", anyhow_err);
    ApiError::InternalServerError(anyhow_err.into())
}

pub fn bad_request(e: impl Into<anyhow::Error>) -> ApiError {
    let anyhow_err = e.into();
    info!("{}", anyhow_err);
    ApiError::BadRequest(anyhow_err.into())
}

#[allow(clippy::all)]
pub mod codegen {
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/mod.rs"));
}

pub(crate) fn empty_string_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: std::fmt::Display,
{
    let opt = Option::<String>::deserialize(de)?;
    match opt.as_deref() {
        None | Some("") => Ok(None),
        Some(s) => FromStr::from_str(s)
            .map_err(serde::de::Error::custom)
            .map(Some),
    }
}
