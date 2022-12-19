//! Prometheus http exporter

use std::process;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use futures::future::Shared;
use futures::Future;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use log::info;
use once_cell::sync::{Lazy, OnceCell};
use prometheus::{self, register_gauge, Encoder, Gauge, TextEncoder};

use crate::api::LightningInterface;

static START: OnceCell<Instant> = OnceCell::new();

static UPTIME: Lazy<Gauge> = Lazy::new(|| {
    register_gauge!(
        "lightning_knd_uptime",
        "Time in milliseconds how long daemon is running"
    )
    .unwrap()
});

static NODE_COUNT: Lazy<Gauge> = Lazy::new(|| {
    register_gauge!(
        "lightning_node_count",
        "The number of nodes in the lightning graph"
    )
    .unwrap()
});

static CHANNEL_COUNT: Lazy<Gauge> = Lazy::new(|| {
    register_gauge!(
        "lightning_channel_count",
        "The number of channels in the lightning graph"
    )
    .unwrap()
});

static PEER_COUNT: Lazy<Gauge> = Lazy::new(|| {
    register_gauge!("lightning_peer_count", "The number of peers this node has").unwrap()
});

static WALLET_BALANCE: Lazy<Gauge> =
    Lazy::new(|| register_gauge!("wallet_balance", "The bitcoin wallet balance").unwrap());

async fn response_examples(
    lightning_metrics: Arc<dyn LightningInterface + Send + Sync>,
    req: Request<Body>,
) -> hyper::Result<Response<Body>> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/health") => Ok(Response::new(Body::from("OK"))),
        (&Method::GET, "/pid") => Ok(Response::new(Body::from(process::id().to_string()))),
        (&Method::GET, "/metrics") => {
            UPTIME.set(START.get().unwrap().elapsed().as_millis() as f64);
            NODE_COUNT.set(lightning_metrics.graph_num_nodes() as f64);
            CHANNEL_COUNT.set(lightning_metrics.graph_num_channels() as f64);
            PEER_COUNT.set(lightning_metrics.num_peers() as f64);
            WALLET_BALANCE.set(lightning_metrics.wallet_balance() as f64);
            let metric_families = prometheus::gather();
            let mut buffer = vec![];
            let encoder = TextEncoder::new();
            encoder.encode(&metric_families, &mut buffer).unwrap();
            Ok(Response::new(Body::from(buffer)))
        }
        _ => Ok(not_found()),
    }
}

static NOTFOUND: &[u8] = b"Not Found";
/// HTTP status code 404
fn not_found() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(NOTFOUND.into())
        .unwrap()
}

/// Starts an prometheus exporter backend
pub async fn start_prometheus_exporter(
    address: String,
    lightning_metrics: Arc<dyn LightningInterface + Send + Sync>,
    quit_signal: Shared<impl Future<Output = ()>>,
) -> Result<()> {
    START.set(Instant::now()).unwrap();
    let addr = address.parse().context("Failed to parse exporter")?;
    let make_service = make_service_fn(move |_| {
        let lightning_metrics_clone = lightning_metrics.clone();
        let service =
            service_fn(move |req| response_examples(lightning_metrics_clone.clone(), req));
        async move { Ok::<_, hyper::Error>(service) }
    });

    let server = Server::bind(&addr)
        .serve(make_service)
        .with_graceful_shutdown(quit_signal);

    info!("Prometheus exporter listening on http://{}", addr);

    server.await.context("Failed to start server")
}
