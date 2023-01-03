mod channels;
mod lightning_interface;
mod macaroon_auth;
mod methods;
mod wallet;
mod wallet_interface;

use anyhow::Result;
use axum::{extract::Extension, response::IntoResponse, routing::get, Router};
use axum_server::{tls_rustls::RustlsConfig, Handle};
use futures::{future::Shared, Future};
use hyper::StatusCode;
pub use lightning_interface::LightningInterface;
use log::{error, info};
pub use macaroon_auth::{KndMacaroon, MacaroonAuth};
use std::{sync::Arc, time::Duration};
use tower_http::cors::CorsLayer;
pub use wallet_interface::WalletInterface;

use self::methods::get_info;
use crate::api::{channels::list_channels, wallet::get_balance};

pub async fn start_rest_api(
    listen_address: String,
    certs_dir: String,
    lightning_api: Arc<dyn LightningInterface + Send + Sync>,
    wallet_api: Arc<dyn WalletInterface + Send + Sync>,
    macaroon_auth: Arc<MacaroonAuth>,
    quit_signal: Shared<impl Future<Output = ()>>,
) -> Result<()> {
    info!("Starting REST API");
    let rustls_config = config(&certs_dir).await;
    let cors = CorsLayer::permissive();
    let handle = Handle::new();

    let app = Router::new()
        .route("/", get(root))
        .route("/v1/getinfo", get(get_info))
        .route("/v1/getbalance", get(get_balance))
        .route("/v1/channel/listChannels", get(list_channels))
        .fallback(handler_404)
        .layer(cors)
        .layer(Extension(lightning_api))
        .layer(Extension(wallet_api))
        .layer(Extension(macaroon_auth));

    let addr = listen_address.parse()?;

    tokio::select!(
        result = axum_server::bind_rustls(addr, rustls_config)
            .serve(app.into_make_service()) => {
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
    info!("STOPPED");
    Ok(())
}

async fn root(
    macaroon: KndMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
) -> Result<impl IntoResponse, StatusCode> {
    if macaroon_auth.verify_macaroon(&macaroon.0).is_err() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok("OK")
}

async fn handler_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "No such method.")
}

async fn config(certs_dir: &str) -> RustlsConfig {
    RustlsConfig::from_pem_file(
        format!("{}/knd.crt", certs_dir),
        format!("{}/knd.key", certs_dir),
    )
    .await
    .unwrap()
}

#[macro_export]
macro_rules! to_string_empty {
    ($v: expr) => {
        $v.map_or("".to_string(), |x| x.to_string())
    };
}
