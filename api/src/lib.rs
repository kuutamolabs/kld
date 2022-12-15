mod get_info;
mod lightning_interface;
mod macaroon_auth;

use get_info::get_info;
pub use get_info::{Chain, GetInfo};
pub use lightning_interface::LightningInterface;
pub use macaroon_auth::MacaroonAuth;

use std::sync::Arc;

use anyhow::Result;
use axum::{extract::Extension, routing::get, Router};
use futures::{future::Shared, Future};

pub async fn start_rest_api(
    listen_address: String,
    lightning_api: Arc<dyn LightningInterface + Send + Sync>,
    macaroon_auth: Arc<MacaroonAuth>,
    quit_signal: Shared<impl Future<Output = ()>>,
) -> Result<()> {
    let app = Router::new()
        .route("/", get(root))
        .route("/v1/getinfo", get(get_info))
        .layer(Extension(lightning_api))
        .layer(Extension(macaroon_auth));
    let addr = listen_address.parse()?;
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(quit_signal)
        .await
        .unwrap();
    Ok(())
}

async fn root() -> &'static str {
    "OK"
}
