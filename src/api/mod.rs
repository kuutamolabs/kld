mod channels;
mod lightning_interface;
mod macaroon_auth;
mod methods;
mod network;
mod peers;
mod wallet;
mod wallet_interface;
mod ws;

pub use lightning_interface::{LightningInterface, OpenChannelResult, Peer, PeerStatus};
pub use macaroon_auth::{KndMacaroon, MacaroonAuth};
pub use wallet_interface::WalletInterface;

use self::methods::get_info;
use crate::api::{
    channels::{close_channel, list_channels, open_channel, set_channel_fee},
    network::{get_node, list_nodes},
    peers::{connect_peer, disconnect_peer, list_peers},
    wallet::{get_balance, new_address, transfer},
    ws::ws_handler,
};
use anyhow::{Context, Result};
use api::routes;
use axum::{
    extract::Extension,
    response::IntoResponse,
    routing::{delete, get, post},
    Router,
};
use axum_server::{
    tls_rustls::{RustlsAcceptor, RustlsConfig},
    Handle, Server,
};
use futures::{future::Shared, Future};
use hyper::StatusCode;
use log::{error, info};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tower_http::cors::CorsLayer;

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
        lightning_api: Arc<dyn LightningInterface + Send + Sync>,
        wallet_api: Arc<dyn WalletInterface + Send + Sync>,
        macaroon_auth: Arc<MacaroonAuth>,
        quit_signal: Shared<impl Future<Output = ()>>,
    ) -> Result<()> {
        let cors = CorsLayer::permissive();
        let handle = Handle::new();

        let app = Router::new()
            .route(routes::ROOT, get(root))
            .route(routes::GET_INFO, get(get_info))
            .route(routes::GET_BALANCE, get(get_balance))
            .route(routes::LIST_CHANNELS, get(list_channels))
            .route(routes::OPEN_CHANNEL, post(open_channel))
            .route(routes::SET_CHANNEL_FEE, post(set_channel_fee))
            .route(routes::CLOSE_CHANNEL, delete(close_channel))
            .route(routes::NEW_ADDR, get(new_address))
            .route(routes::WITHDRAW, post(transfer))
            .route(routes::LIST_PEERS, get(list_peers))
            .route(routes::CONNECT_PEER, post(connect_peer))
            .route(routes::DISCONNECT_PEER, delete(disconnect_peer))
            .route(routes::LIST_NODES, get(list_nodes))
            .route(routes::LIST_NODE, get(get_node))
            .route(routes::WEBSOCKET, get(ws_handler))
            .fallback(handler_404)
            .layer(cors)
            .layer(Extension(lightning_api))
            .layer(Extension(wallet_api))
            .layer(Extension(macaroon_auth));

        tokio::select!(
            result = self.server.serve(app.into_make_service_with_connect_info::<SocketAddr>()) => {
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

async fn root(
    macaroon: KndMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
) -> Result<impl IntoResponse, StatusCode> {
    if macaroon_auth.verify_readonly_macaroon(&macaroon.0).is_err() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok("OK")
}

async fn handler_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "No such method.")
}

async fn config(certs_dir: &str) -> Result<RustlsConfig> {
    RustlsConfig::from_pem_file(
        format!("{certs_dir}/knd.crt"),
        format!("{certs_dir}/knd.key"),
    )
    .await
    .context("failed to load certificates")
}

#[macro_export]
macro_rules! to_string_empty {
    ($v: expr) => {
        $v.map_or("".to_string(), |x| x.to_string())
    };
}

#[macro_export]
macro_rules! handle_err {
    ($parse:expr) => {
        $parse.map_err(|e| {
            warn!("{}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    };
}

#[macro_export]
macro_rules! handle_unauthorized {
    ($parse:expr) => {
        $parse.map_err(|e| {
            info!("{}", e);
            StatusCode::UNAUTHORIZED
        })?
    };
}

#[macro_export]
macro_rules! handle_bad_request {
    ($parse:expr) => {
        $parse.map_err(|e| {
            info!("{}", e);
            StatusCode::BAD_REQUEST
        })?
    };
}
