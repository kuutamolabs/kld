use anyhow::{Context, Result};
use futures::FutureExt;
use std::sync::Arc;
use test_utils::{test_settings, poll};

use crate::{mocks::mock_lightning::MockLightning, quit_signal};
use kld::prometheus::start_prometheus_exporter;

#[tokio::test(flavor = "multi_thread")]
pub async fn test_prometheus() -> Result<()> {
    let address = test_settings().exporter_address.clone();
    println!("ADDRESS: {address}");

    let metrics = Arc::new(MockLightning::default());
    tokio::spawn(start_prometheus_exporter(
        address.clone(),
        metrics.clone(),
        quit_signal().shared(),
    ));
    poll!(3, call_exporter(&address, "health").await.is_ok());

    let health = call_exporter(&address, "health").await?;
    assert_eq!(health, "OK");

    let pid = call_exporter(&address, "pid").await?;
    assert_eq!(pid, std::process::id().to_string());

    let result = call_exporter(&address, "metrics").await?;
    assert!(get_metric(&result, "uptime")?.is_finite());
    assert_eq!(get_metric(&result, "node_count")?, metrics.num_nodes as f64);
    assert_eq!(
        get_metric(&result, "channel_count")?,
        metrics.num_channels as f64
    );
    assert_eq!(get_metric(&result, "peer_count")?, metrics.num_peers as f64);
    assert_eq!(
        get_metric(&result, "wallet_balance")?,
        metrics.wallet_balance as f64
    );

    let not_found = call_exporter(&address, "wrong").await?;
    assert_eq!(not_found, "Not Found");
    Ok(())
}

async fn call_exporter(address: &str, method: &str) -> Result<String, reqwest::Error> {
    reqwest::get(format!("http://{address}/{method}"))
        .await?
        .text()
        .await
}

fn get_metric(metrics: &str, name: &str) -> Result<f64> {
    Ok(metrics
        .lines()
        .find(|x| x.starts_with(name))
        .with_context(|| "Metric not found")?
        .split(' ')
        .last()
        .with_context(|| "Bad metric format")?
        .parse::<f64>()?)
}
