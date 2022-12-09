//! Prometheus http exporter

use std::process;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use api::LightningInterface;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use log::info;
use once_cell::sync::{Lazy, OnceCell};
use prometheus::{self, register_gauge, Encoder, Gauge, TextEncoder};

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
            NODE_COUNT.set(lightning_metrics.num_nodes() as f64);
            CHANNEL_COUNT.set(lightning_metrics.num_channels() as f64);
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
pub(crate) async fn start_prometheus_exporter(
    address: String,
    lightning_metrics: Arc<dyn LightningInterface + Send + Sync>,
) -> Result<()> {
    START.set(Instant::now()).unwrap();
    let addr = address.parse().context("Failed to parse exporter")?;
    let make_service = make_service_fn(move |_| {
        let lightning_metrics_clone = lightning_metrics.clone();
        let service =
            service_fn(move |req| response_examples(lightning_metrics_clone.clone(), req));
        async move { Ok::<_, hyper::Error>(service) }
    });

    let server = Server::bind(&addr).serve(make_service);

    info!("Prometheus exporter listening on http://{}", addr);

    server.await.context("Failed to start server")
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use anyhow::Result;
    use api::LightningInterface;
    use bitcoin::{secp256k1::PublicKey, Network};
    use test_utils::{random_public_key, test_settings};

    use crate::start_prometheus_exporter;

    struct TestMetrics {
        num_nodes: usize,
        num_channels: usize,
        num_peers: usize,
        wallet_balance: u64,
    }

    impl LightningInterface for TestMetrics {
        fn alias(&self) -> String {
            "test".to_string()
        }
        fn identity_pubkey(&self) -> PublicKey {
            random_public_key()
        }

        fn num_nodes(&self) -> usize {
            self.num_nodes
        }

        fn num_channels(&self) -> usize {
            self.num_channels
        }

        fn block_height(&self) -> usize {
            50000
        }

        fn network(&self) -> bitcoin::Network {
            Network::Bitcoin
        }
        fn num_active_channels(&self) -> usize {
            0
        }

        fn num_inactive_channels(&self) -> usize {
            0
        }

        fn num_pending_channels(&self) -> usize {
            0
        }
        fn num_peers(&self) -> usize {
            self.num_peers
        }

        fn wallet_balance(&self) -> u64 {
            self.wallet_balance
        }

        fn version(&self) -> String {
            "v0.1".to_string()
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_prometheus() {
        let address = test_settings().exporter_address.clone();

        let metrics = Arc::new(TestMetrics {
            num_nodes: 10,
            num_channels: 20,
            num_peers: 5,
            wallet_balance: 500000,
        });
        tokio::spawn(start_prometheus_exporter(address.clone(), metrics.clone()));

        let health = call_exporter(&address, "health").await.unwrap();
        assert_eq!(health, "OK");

        let pid = call_exporter(&address, "pid").await.unwrap();
        assert_eq!(pid, std::process::id().to_string());

        let result = call_exporter(&address, "metrics").await.unwrap();
        assert!(get_metric(&result, "lightning_knd_uptime") > 0.0);
        assert_eq!(
            get_metric(&result, "lightning_node_count"),
            metrics.num_nodes as f64
        );
        assert_eq!(
            get_metric(&result, "lightning_channel_count"),
            metrics.num_channels as f64
        );
        assert_eq!(
            get_metric(&result, "lightning_peer_count"),
            metrics.num_peers as f64
        );
        assert_eq!(
            get_metric(&result, "wallet_balance"),
            metrics.wallet_balance as f64
        );

        let not_found = call_exporter(&address, "wrong").await.unwrap();
        assert_eq!(not_found, "Not Found");
    }

    async fn call_exporter(address: &str, method: &str) -> Result<String, reqwest::Error> {
        reqwest::get(format!("http://{}/{}", address, method))
            .await?
            .text()
            .await
    }

    fn get_metric(metrics: &str, name: &str) -> f64 {
        metrics
            .lines()
            .find(|x| x.starts_with(name))
            .unwrap()
            .split(' ')
            .last()
            .unwrap()
            .parse::<f64>()
            .unwrap()
    }
}
