//! Prometheus http exporter

use std::process;
use std::time::Instant;

use anyhow::{Context, Result};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use lazy_static::lazy_static;
use log::info;
use prometheus::{self, register_gauge, Encoder, Gauge, TextEncoder};

lazy_static! {
    static ref START: Instant = Instant::now();
    static ref UPTIME: Gauge = register_gauge!(
        "lightning_knd_uptime",
        "Time in milliseconds how long daemon is running"
    )
    .unwrap();
}

async fn response_examples(req: Request<Body>) -> hyper::Result<Response<Body>> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/health") => Ok(Response::new(Body::from("OK"))),
        (&Method::GET, "/pid") => Ok(Response::new(Body::from(process::id().to_string()))),
        (&Method::GET, "/metrics") => {
            UPTIME.set(START.elapsed().as_millis() as f64);

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
pub async fn spawn_prometheus_exporter(exporter_address: &str) -> Result<()> {
    lazy_static::initialize(&START);
    let addr = exporter_address
        .parse()
        .context("Failed to parse exporter")?;
    let make_service =
        make_service_fn(|_| async { Ok::<_, hyper::Error>(service_fn(response_examples)) });

    let server = Server::bind(&addr).serve(make_service);

    info!("Listening on http://{}", addr);

    server.await.context("Failed to start server")
}
