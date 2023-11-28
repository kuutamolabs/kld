//! Prometheus http exporter

use std::process;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use anyhow::{Context, Result};
use futures::future::Shared;
use futures::Future;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use lightning::chain::chaininterface::ConfirmationTarget;
use log::info;
use prometheus::{self, register_gauge, register_int_gauge, Encoder, Gauge, IntGauge, TextEncoder};

use crate::bitcoind::BitcoindMetrics;
use crate::database::DBConnection;
use crate::ldk::LightningInterface;
use crate::settings::Settings;

static START: OnceLock<Instant> = OnceLock::new();
static UPTIME: OnceLock<Gauge> = OnceLock::new();
/// The free balance without any channel setting
static WALLET_BALANCE: OnceLock<Gauge> = OnceLock::new();
/// The balance already used in channel and bond in our side
static CHANNEL_BALANCE: OnceLock<Gauge> = OnceLock::new();
static FEE: OnceLock<Gauge> = OnceLock::new();
static BLOCK_HEIGHT: OnceLock<IntGauge> = OnceLock::new();

static ON_CHAIN_SWEEP_FEE: OnceLock<IntGauge> = OnceLock::new();
static MAX_ALLOWED_NON_ANCHOR_CHANNEL_REMOTE_FEE: OnceLock<IntGauge> = OnceLock::new();
static NON_ANCHOR_CHANNEL_FEE: OnceLock<IntGauge> = OnceLock::new();
static CHANNEL_CLOSE_MINIMUM_FEE: OnceLock<IntGauge> = OnceLock::new();
static ANCHOR_CHANNEL_FEE: OnceLock<IntGauge> = OnceLock::new();
static MIN_ALLOWED_ANCHOR_CHANNEL_REMOTE_FEE: OnceLock<IntGauge> = OnceLock::new();
static MIN_ALLOWED_NON_ANCHOR_CHANNEL_REMOTE_FEE: OnceLock<IntGauge> = OnceLock::new();
static SCORER_UPDATE_TIMESTAMP: OnceLock<IntGauge> = OnceLock::new();
static PROBE_INTERVAL: OnceLock<IntGauge> = OnceLock::new();
static PROBE_AMOUNT: OnceLock<Gauge> = OnceLock::new();

// NOTE:
// Gauge will slow down about 20%~30%, unleast the count reach the limit, else we
// should use IntGauge
static NODE_COUNT: OnceLock<IntGauge> = OnceLock::new();
static NETWORK_CHANNEL_COUNT: OnceLock<IntGauge> = OnceLock::new();
static CHANNEL_COUNT: OnceLock<IntGauge> = OnceLock::new();
static PEER_COUNT: OnceLock<IntGauge> = OnceLock::new();

async fn response_examples(
    lightning_metrics: Arc<dyn LightningInterface>,
    database: Arc<dyn DBConnection>,
    bitcoind: Arc<dyn BitcoindMetrics>,
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
                g.set(
                    lightning_metrics
                        .graph_num_nodes()
                        .try_into()
                        .unwrap_or(i64::MAX),
                )
            }
            if let Some(g) = NETWORK_CHANNEL_COUNT.get() {
                g.set(
                    lightning_metrics
                        .graph_num_channels()
                        .try_into()
                        .unwrap_or(i64::MAX),
                )
            }
            if let (Some(g), Ok(i)) = (CHANNEL_COUNT.get(), database.open_channel_count().await) {
                g.set(i);
            }
            if let Some(g) = PEER_COUNT.get() {
                g.set(lightning_metrics.num_peers().try_into().unwrap_or(i64::MAX))
            }
            if let Some(g) = WALLET_BALANCE.get() {
                g.set(lightning_metrics.wallet_balance() as f64)
            }
            if let Some(g) = CHANNEL_BALANCE.get() {
                let mut total_channel_balance = 0;
                let channels = lightning_metrics.list_channels();
                for channel in channels {
                    total_channel_balance += channel.balance_msat;
                }
                g.set((total_channel_balance as f64) / 1000.0)
            }
            // XXX better from dbconnection not lightning_metrics, if the fee is get from database
            if let (Some(g), Ok(total_fee)) =
                (FEE.get(), lightning_metrics.fetch_total_forwards().await)
            {
                g.set(total_fee.fee as f64)
            }
            if let (Some(g), Ok(h)) = (BLOCK_HEIGHT.get(), bitcoind.block_height().await) {
                g.set(h.into())
            }

            if let Some(g) = ON_CHAIN_SWEEP_FEE.get() {
                g.set(bitcoind.fee_for(ConfirmationTarget::OnChainSweep).into())
            }
            if let Some(g) = MAX_ALLOWED_NON_ANCHOR_CHANNEL_REMOTE_FEE.get() {
                g.set(
                    bitcoind
                        .fee_for(ConfirmationTarget::MaxAllowedNonAnchorChannelRemoteFee)
                        .into(),
                )
            }
            if let Some(g) = NON_ANCHOR_CHANNEL_FEE.get() {
                g.set(
                    bitcoind
                        .fee_for(ConfirmationTarget::NonAnchorChannelFee)
                        .into(),
                )
            }
            if let Some(g) = CHANNEL_CLOSE_MINIMUM_FEE.get() {
                g.set(
                    bitcoind
                        .fee_for(ConfirmationTarget::ChannelCloseMinimum)
                        .into(),
                )
            }
            if let Some(g) = ANCHOR_CHANNEL_FEE.get() {
                g.set(
                    bitcoind
                        .fee_for(ConfirmationTarget::AnchorChannelFee)
                        .into(),
                )
            }
            if let Some(g) = MIN_ALLOWED_ANCHOR_CHANNEL_REMOTE_FEE.get() {
                g.set(
                    bitcoind
                        .fee_for(ConfirmationTarget::MinAllowedAnchorChannelRemoteFee)
                        .into(),
                )
            }
            if let Some(g) = MIN_ALLOWED_NON_ANCHOR_CHANNEL_REMOTE_FEE.get() {
                g.set(
                    bitcoind
                        .fee_for(ConfirmationTarget::MinAllowedNonAnchorChannelRemoteFee)
                        .into(),
                )
            }
            if let (Some(g), Ok(ts)) = (
                SCORER_UPDATE_TIMESTAMP.get(),
                database.fetch_scorer_update_time().await,
            ) {
                g.set(ts.unix_timestamp());
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
    settings: Arc<Settings>,
    lightning_metrics: Arc<dyn LightningInterface>,
    database: Arc<dyn DBConnection>,
    bitcoind: Arc<dyn BitcoindMetrics>,
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
    CHANNEL_BALANCE
        .set(register_gauge!(
            "channel_balance",
            "The bitcoin balance in channel and in our side"
        )?)
        .unwrap_or_default();
    FEE.set(register_gauge!(
        "fee",
        "The total fee from successful channels"
    )?)
    .unwrap_or_default();
    BLOCK_HEIGHT
        .set(register_int_gauge!(
            "block_height",
            "The block height kld observed"
        )?)
        .unwrap_or_default();
    ON_CHAIN_SWEEP_FEE
        .set(register_int_gauge!(
            "on_chain_sweep_fee",
            "The fee for ConfirmationTarget::OnChainSweep"
        )?)
        .unwrap_or_default();
    MAX_ALLOWED_NON_ANCHOR_CHANNEL_REMOTE_FEE
        .set(register_int_gauge!(
            "max_allowed_non_anchor_channel_remote_fee",
            "The fee for ConfirmationTarget::MaxAllowedNonAnchorChannelRemoteFee"
        )?)
        .unwrap_or_default();
    NON_ANCHOR_CHANNEL_FEE
        .set(register_int_gauge!(
            "non_anchor_channel_fee",
            "The fee for ConfirmationTarget::NonAnchorChannelFee"
        )?)
        .unwrap_or_default();
    CHANNEL_CLOSE_MINIMUM_FEE
        .set(register_int_gauge!(
            "channel_close_minimum_fee",
            "The fee for ConfirmationTarget::ChannelCloseMinimum"
        )?)
        .unwrap_or_default();
    ANCHOR_CHANNEL_FEE
        .set(register_int_gauge!(
            "anchor_channel_fee",
            "The fee for ConfirmationTarget::AnchorChannelFee"
        )?)
        .unwrap_or_default();
    MIN_ALLOWED_ANCHOR_CHANNEL_REMOTE_FEE
        .set(register_int_gauge!(
            "min_allowed_anchor_channel_remote_fee",
            "The fee for ConfirmationTarget::MinAllowedAnchorChannelRemoteFee"
        )?)
        .unwrap_or_default();
    MIN_ALLOWED_NON_ANCHOR_CHANNEL_REMOTE_FEE
        .set(register_int_gauge!(
            "min_allowed_non_anchor_channel_remote_fee",
            "The fee for ConfirmationTarget::MinAllowedNonAnchorChannelRemoteFee"
        )?)
        .unwrap_or_default();
    SCORER_UPDATE_TIMESTAMP
        .set(register_int_gauge!(
            "scorer_update_timestamp",
            "The update time of scorer"
        )?)
        .unwrap_or_default();
    PROBE_INTERVAL
        .set(register_int_gauge!(
            "probe_interval",
            "The interval of probe in seconds"
        )?)
        .unwrap_or_default();
    if let Some(g) = PROBE_INTERVAL.get() {
        g.set(settings.probe_interval as i64)
    }
    PROBE_AMOUNT
        .set(register_gauge!(
            "probe_amount",
            "The amount in BTC for each probe"
        )?)
        .unwrap_or_default();
    if let Some(g) = PROBE_AMOUNT.get() {
        g.set(settings.probe_amt_msat as f64 * 0.01)
    }

    let addr = settings
        .exporter_address
        .parse()
        .context("Failed to parse exporter")?;
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
