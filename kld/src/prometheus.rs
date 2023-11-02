//! Prometheus http exporter

use std::process;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use anyhow::{Context, Result};
use futures::future::Shared;
use futures::Future;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use log::info;
use prometheus::{self, register_gauge, register_int_gauge, Encoder, IntGauge, Gauge, TextEncoder};

use crate::ldk::LightningInterface;
use crate::Service;

static START: OnceLock<Instant> = OnceLock::new();
static UPTIME: OnceLock<Gauge> = OnceLock::new();
static WALLET_BALANCE: OnceLock<Gauge> = OnceLock::new();

// NOTE:
// Gauge will slow down about 20%~30%, unleast the count reach the limit, else we
// should use IntGauge
static NODE_COUNT: OnceLock<IntGauge> = OnceLock::new();
static NETWORK_CHANNEL_COUNT: OnceLock<IntGauge> = OnceLock::new();
static CHANNEL_COUNT: OnceLock<IntGauge> = OnceLock::new();
static PEER_COUNT: OnceLock<IntGauge> = OnceLock::new();

async fn response_examples(
    lightning_metrics: Arc<dyn LightningInterface>,
    database: Arc<dyn Service>,
    bitcoind: Arc<dyn Service>,
    req: Request<Body>,
) -> hyper::Result<Response<Body>> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/health") => {
            let health = if database.is_synchronised().await && bitcoind.is_synchronised().await {
                "OK"
            } else if database.is_connected().await && bitcoind.is_connected().await {
                "SYNCING"
            } else {
                "ERROR"
            };
            Ok(Response::new(Body::from(health)))
        }
        (&Method::GET, "/pid") => Ok(Response::new(Body::from(process::id().to_string()))),
        (&Method::GET, "/metrics") => {
            if let Some(g) = UPTIME.get() {
                g.set(START.get_or_init(Instant::now).elapsed().as_millis() as f64)
            }
            if let Some(g) = NODE_COUNT.get() {
                g.set(lightning_metrics.graph_num_nodes().try_into().unwrap_or(i64::MAX))
            }
            if let Some(g) = NETWORK_CHANNEL_COUNT.get() {
                g.set(lightning_metrics.graph_num_channels().try_into().unwrap_or(i64::MAX))
            }
            if let Some(g) = CHANNEL_COUNT.get() {
                g.set(lightning_metrics.list_channels().len().try_into().unwrap_or(i64::MAX))
            }
            if let Some(g) = PEER_COUNT.get() {
                g.set(lightning_metrics.num_peers().try_into().unwrap_or(i64::MAX))
            }
            if let Some(g) = WALLET_BALANCE.get() {
                g.set(lightning_metrics.wallet_balance() as f64)
            }
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
    lightning_metrics: Arc<dyn LightningInterface>,
    database: Arc<dyn Service>,
    bitcoind: Arc<dyn Service>,
    quit_signal: Shared<impl Future<Output = ()>>,
) -> Result<()> {
    UPTIME
        .set(register_gauge!(
            "uptime",
            "Time in milliseconds how long daemon is running"
        )?)
        .unwrap_or_default();
    NODE_COUNT
        .set(register_int_gauge!(
            "node_count",
            "The number of nodes in the lightning graph"
        )?)
        .unwrap_or_default();
    NETWORK_CHANNEL_COUNT
        .set(register_int_gauge!(
            "network_channel_count",
            "The number of channels in the lightning network"
        )?)
        .unwrap_or_default();
    CHANNEL_COUNT
        .set(register_int_gauge!(
            "channel_count",
            "The number of channels opened by us"
        )?)
        .unwrap_or_default();
    PEER_COUNT
        .set(register_int_gauge!(
            "peer_count",
            "The number of peers this node has"
        )?)
        .unwrap_or_default();
    WALLET_BALANCE
        .set(register_gauge!(
            "wallet_balance",
            "The bitcoin wallet balance"
        )?)
        .unwrap_or_default();

    let addr = address.parse().context("Failed to parse exporter")?;
    let make_service = make_service_fn(move |_| {
        let lightning_metrics_clone = lightning_metrics.clone();
        let bitcoind_clone = bitcoind.clone();
        let database_clone = database.clone();
        let service = service_fn(move |req| {
            response_examples(
                lightning_metrics_clone.clone(),
                database_clone.clone(),
                bitcoind_clone.clone(),
                req,
            )
        });
        async move { Ok::<_, hyper::Error>(service) }
    });

    let server = Server::bind(&addr)
        .serve(make_service)
        .with_graceful_shutdown(quit_signal);

    info!("Prometheus exporter listening on http://{}", addr);

    server.await.context("Failed to start server")
}
