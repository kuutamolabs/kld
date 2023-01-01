mod lightning_interface;
mod macaroon_auth;
mod methods;

use axum_server::{tls_rustls::RustlsConfig, Handle};
pub use lightning_interface::LightningInterface;
use log::{error, info};
pub use macaroon_auth::{KndMacaroon, MacaroonAuth};
use tower_http::cors::CorsLayer;

use std::{sync::Arc, time::Duration};

use anyhow::Result;
use axum::{extract::Extension, routing::get, Router};
use futures::{future::Shared, Future};

use self::methods::get_info;

pub async fn start_rest_api(
    listen_address: String,
    certs_dir: String,
    lightning_api: Arc<dyn LightningInterface + Send + Sync>,
    macaroon_auth: Arc<MacaroonAuth>,
    quit_signal: Shared<impl Future<Output = ()>>,
) -> Result<()> {
    let rustls_config = config(&certs_dir).await;
    let cors = CorsLayer::permissive();
    let handle = Handle::new();

    let app = Router::new()
        .route("/", get(root))
        .route("/v1/getinfo", get(get_info))
        .layer(cors)
        .layer(Extension(lightning_api))
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

    Ok(())
}

async fn root() -> &'static str {
    "OK"
}

async fn config(certs_dir: &str) -> RustlsConfig {
    RustlsConfig::from_pem_file(
        format!("{}/knd.crt", certs_dir),
        format!("{}/knd.key", certs_dir),
    )
    .await
    .unwrap()
}
