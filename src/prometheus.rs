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

use crate::controller::Controller;
use crate::settings::Settings;

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
    controller: Arc<Controller>,
    req: Request<Body>,
) -> hyper::Result<Response<Body>> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/health") => Ok(Response::new(Body::from("OK"))),
        (&Method::GET, "/pid") => Ok(Response::new(Body::from(process::id().to_string()))),
        (&Method::GET, "/metrics") => {
            UPTIME.set(START.elapsed().as_millis() as f64);
            NODE_COUNT.set(controller.num_nodes() as f64);
            CHANNEL_COUNT.set(controller.num_channels() as f64);
            PEER_COUNT.set(controller.num_peers() as f64);

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
    settings: &Settings,
    controller: Arc<Controller>,
) -> Result<()> {
    lazy_static::initialize(&START);
    let addr = settings
        .exporter_address
        .parse()
        .context("Failed to parse exporter")?;
    let make_service = make_service_fn(move |_| {
        let controller_clone = controller.clone();
        let service = service_fn(move |req| response_examples(controller_clone.clone(), req));
        async move { Ok::<_, hyper::Error>(service) }
    });

    let server = Server::bind(&addr).serve(make_service);

    info!("Listening on http://{}", addr);

    server.await.context("Failed to start server")
}
