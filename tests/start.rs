use std::{str::FromStr, time::Duration};

use anyhow::Result;
use api::{routes, GetInfo};
use bitcoin::Address;
use kld::bitcoind::BitcoindClient;
use test_utils::{
    bitcoin, bitcoin_manager::BitcoinManager, cockroach, kld, teos, TestSettingsBuilder,
};
use tokio::time::{sleep_until, Instant};

// This test is run separately (in its own process) from the other threads.
// As it starts all the services it might clash with other tests.
#[tokio::test(flavor = "multi_thread")]
pub async fn test_start() -> Result<()> {
    let mut cockroach = cockroach!();
    cockroach.start().await?;
    let mut bitcoin = bitcoin!();
    bitcoin.start().await?;
    let n_blocks = 6;
    generate_blocks(&bitcoin, n_blocks).await?;

    let mut teos = teos!(&bitcoin);
    teos.start().await?;

    let mut kld = kld!(&bitcoin, &cockroach);
    kld.start().await?;

    let health = kld.call_exporter("health").await.unwrap();
    assert_eq!(health, "OK");
    let pid = kld.call_exporter("pid").await.unwrap();
    assert_eq!(pid, kld.pid().unwrap().to_string());
    assert!(kld.call_exporter("metrics").await.is_ok());

    assert_eq!("OK", kld.call_rest_api("").await.unwrap());

    let result = kld.call_rest_api(routes::GET_INFO).await.unwrap();
    let info: GetInfo = serde_json::from_str(&result).unwrap();
    assert_eq!(n_blocks, info.block_height);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Only run this for manual testing"]
pub async fn test_manual() -> Result<()> {
    let mut cockroach = cockroach!();
    cockroach.start().await?;
    let mut bitcoin = bitcoin!();
    bitcoin.start().await?;
    generate_blocks(&bitcoin, 3).await?;
    let mut kld = kld!(&bitcoin, &cockroach);
    kld.start().await?;

    sleep_until(Instant::now() + Duration::from_secs(10000)).await;
    Ok(())
}

async fn generate_blocks(bitcoin: &BitcoinManager, n_blocks: u64) -> Result<()> {
    let settings = TestSettingsBuilder::new().with_bitcoind(bitcoin)?.build();
    let bitcoin_client = &BitcoindClient::new(&settings).await?;

    bitcoin_client
        .generate_to_address(
            n_blocks,
            &Address::from_str("2N4eQYCbKUHCCTUjBJeHcJp9ok6J2GZsTDt")?,
        )
        .await?;
    bitcoin_client.wait_for_blockchain_synchronisation().await?;
    Ok(())
}
