use std::time::Duration;

use crate::smoke::{start_all, START_N_BLOCKS};
use anyhow::Result;
use api::{routes, GetInfo};
use tokio::time::{sleep_until, Instant};

// This test is run separately (in its own process) from the other threads.
// As it starts all the services it might clash with other tests.
#[tokio::test(flavor = "multi_thread")]
pub async fn test_start() -> Result<()> {
    let (_cockroach, _bitcoin, kld) = start_all("start").await?;

    let health = kld.call_exporter("health").await.unwrap();
    assert_eq!(health, "OK");
    let pid = kld.call_exporter("pid").await.unwrap();
    assert_eq!(pid, kld.pid().unwrap().to_string());
    assert!(kld.call_exporter("metrics").await.is_ok());

    assert!(kld.call_rest_api(routes::ROOT).await.is_ok());

    let result = kld.call_rest_api(routes::GET_INFO).await.unwrap();
    let info: GetInfo = serde_json::from_str(&result).unwrap();
    assert_eq!(START_N_BLOCKS, info.block_height);
    assert!(info.synced_to_chain);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Only run this for manual testing"]
pub async fn test_manual() -> Result<()> {
    start_all("maunal").await?;

    sleep_until(Instant::now() + Duration::from_secs(10000)).await;
    Ok(())
}
