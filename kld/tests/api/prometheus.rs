use anyhow::{Context, Result};
use async_trait::async_trait;
use futures::FutureExt;
use lightning::chain::chaininterface::ConfirmationTarget;
use prometheus::IntCounter;
use std::sync::Arc;
use std::sync::OnceLock;
use test_utils::{poll, ports::get_available_port};
use time::OffsetDateTime;

use crate::{mocks::mock_lightning::MockLightning, quit_signal};
use kld::{
    bitcoind::BitcoindMetrics, database::DBConnection, prometheus::start_prometheus_exporter,
    Service,
};

static PROBE_TOTAL_COUNT: OnceLock<IntCounter> = OnceLock::new();
static PROBE_SUCCESSFUL_COUNT: OnceLock<IntCounter> = OnceLock::new();
static PROBE_FAILED_COUNT: OnceLock<IntCounter> = OnceLock::new();

#[tokio::test(flavor = "multi_thread")]
pub async fn test_prometheus() -> Result<()> {
    let port = get_available_port().context("no port")?;
    let address = format!("127.0.0.1:{port}");

    let metrics = Arc::new(MockLightning::default());

    let database = Arc::new(MockService(true, true));
    let bitcoind = Arc::new(MockService(true, false));

    tokio::spawn(start_prometheus_exporter(
        address.clone(),
        metrics.clone(),
        database,
        bitcoind,
        quit_signal().shared(),
        (
            &PROBE_TOTAL_COUNT,
            &PROBE_SUCCESSFUL_COUNT,
            &PROBE_FAILED_COUNT,
        ),
    ));
    poll!(3, call_exporter(&address, "health").await.is_ok());

    let health = call_exporter(&address, "health").await?;
    assert_eq!(health, "SYNCING");

    let pid = call_exporter(&address, "pid").await?;
    assert_eq!(pid, std::process::id().to_string());

    let result = call_exporter(&address, "metrics").await?;
    assert!(get_metric(&result, "uptime").is_ok());
    assert_eq!(
        get_metric(&result, "node_count")?,
        format!("{}", metrics.num_nodes)
    );
    assert_eq!(
        get_metric(&result, "network_channel_count")?,
        format!("{}", metrics.num_channels)
    );
    assert_eq!(get_metric(&result, "channel_count")?, "1".to_string());
    assert_eq!(
        get_metric(&result, "peer_count")?,
        format!("{}", metrics.num_peers)
    );
    assert_eq!(
        get_metric(&result, "wallet_balance")?,
        format!("{}", metrics.wallet_balance)
    );
    assert_eq!(get_metric(&result, "channel_count")?, "1".to_string());
    assert_eq!(
        get_metric(&result, "channel_balance")?,
        format!("{}", metrics.channel.balance_msat / 1000)
    );
    assert_eq!(
        get_metric(&result, "fee")?,
        format!(
            "{}",
            metrics
                .forward
                .fee
                .expect("test should have fee in forward channel")
        )
    );
    assert_eq!(get_metric(&result, "block_height")?, "1000".to_string());

    let not_found = call_exporter(&address, "wrong").await?;
    assert_eq!(not_found, "Not Found");
    Ok(())
}

struct MockService(bool, bool);

#[async_trait]
impl Service for MockService {
    async fn is_connected(&self) -> bool {
        self.0
    }
    async fn is_synchronised(&self) -> bool {
        self.1
    }
}

#[async_trait]
impl DBConnection for MockService {
    async fn open_channel_count(&self) -> Result<i64> {
        Ok(1)
    }
    async fn fetch_scorer_update_time(&self) -> Result<OffsetDateTime> {
        Ok(OffsetDateTime::from_unix_timestamp(0).unwrap())
    }
}

#[async_trait]
impl BitcoindMetrics for MockService {
    async fn block_height(&self) -> Result<u32> {
        Ok(1000)
    }
    fn fee_for(&self, _target: ConfirmationTarget) -> u32 {
        0
    }
}

async fn call_exporter(address: &str, method: &str) -> Result<String, reqwest::Error> {
    reqwest::get(format!("http://{address}/{method}"))
        .await?
        .text()
        .await
}

fn get_metric<'a>(metrics: &'a str, name: &str) -> Result<&'a str> {
    metrics
        .lines()
        .find(|x| x.starts_with(name))
        .with_context(|| "Metric not found")?
        .split(' ')
        .last()
        .with_context(|| "Bad metric format")
}
