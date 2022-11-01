//! Prometheus http exporter

use std::process;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use lazy_static::lazy_static;
use log::info;
use prometheus::{self, register_gauge, Encoder, Gauge, TextEncoder};

use crate::controller::LightningMetrics;

lazy_static! {
    static ref START: Instant = Instant::now();
    static ref UPTIME: Gauge = register_gauge!(
        "lightning_knd_uptime",
        "Time in milliseconds how long daemon is running"
    )
    .unwrap();
    static ref NODE_COUNT: Gauge = register_gauge!(
        "lightning_node_count",
        "The number of nodes in the lightning graph"
    )
    .unwrap();
    static ref CHANNEL_COUNT: Gauge = register_gauge!(
        "lightning_channel_count",
        "The number of channels in the lightning graph"
    )
    .unwrap();
    static ref PEER_COUNT: Gauge =
        register_gauge!("lightning_peer_count", "The number of peers this node has").unwrap();
}

async fn response_examples(
    lightning_metrics: Arc<dyn LightningMetrics + Send + Sync>,
    req: Request<Body>,
) -> hyper::Result<Response<Body>> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/health") => Ok(Response::new(Body::from("OK"))),
        (&Method::GET, "/pid") => Ok(Response::new(Body::from(process::id().to_string()))),
        (&Method::GET, "/metrics") => {
            UPTIME.set(START.elapsed().as_millis() as f64);
            NODE_COUNT.set(lightning_metrics.num_nodes() as f64);
            CHANNEL_COUNT.set(lightning_metrics.num_channels() as f64);
            PEER_COUNT.set(lightning_metrics.num_peers() as f64);

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
pub(crate) async fn spawn_prometheus_exporter(
    address: String,
    lightning_metrics: Arc<dyn LightningMetrics + Send + Sync>,
) -> Result<()> {
    lazy_static::initialize(&START);
    let addr = address.parse().context("Failed to parse exporter")?;
    let make_service = make_service_fn(move |_| {
        let lightning_metrics_clone = lightning_metrics.clone();
        let service =
            service_fn(move |req| response_examples(lightning_metrics_clone.clone(), req));
        async move { Ok::<_, hyper::Error>(service) }
    });

    let server = Server::bind(&addr).serve(make_service);

    info!("Listening on http://{}", addr);

    server.await.context("Failed to start server")
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use crate::{controller::LightningMetrics, spawn_prometheus_exporter};
    use settings::Settings;

    struct TestMetrics {
        num_nodes: usize,
        num_channels: usize,
        num_peers: usize,
    }

    impl LightningMetrics for TestMetrics {
        fn num_nodes(&self) -> usize {
            self.num_nodes
        }

        fn num_channels(&self) -> usize {
            self.num_channels
        }

        fn num_peers(&self) -> usize {
            self.num_peers
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_prometheus() {
        let address = Settings::load().exporter_address;

        let metrics = Arc::new(TestMetrics {
            num_nodes: 10,
            num_channels: 20,
            num_peers: 5,
        });
        tokio::spawn(spawn_prometheus_exporter(address.clone(), metrics.clone()));

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
