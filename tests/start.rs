use std::{
    str::FromStr,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use anyhow::Result;
use api::{routes, GetInfo};
use bitcoin::Address;
use bitcoind::Client;
use test_utils::{bitcoin, cockroach, knd, teos};
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
    let bitcoin_client = Client::new(
        "127.0.0.1".to_string(),
        bitcoin.rpc_port,
        bitcoin.cookie_path(),
        Arc::new(AtomicBool::new(false)),
    )
    .await
    .unwrap();
    bitcoin_client
        .generate_to_address(
            n_blocks as u32,
            &Address::from_str("2N4eQYCbKUHCCTUjBJeHcJp9ok6J2GZsTDt").unwrap(),
        )
        .await;
    bitcoin_client.wait_for_blockchain_synchronisation().await;

    let mut teos = teos!(&bitcoin);
    teos.start().await?;

    let mut knd = knd!(&bitcoin, &cockroach);
    knd.start().await?;

    let health = knd.call_exporter("health").await.unwrap();
    assert_eq!(health, "OK");
    let pid = knd.call_exporter("pid").await.unwrap();
    assert_eq!(pid, knd.pid().unwrap().to_string());
    assert!(knd.call_exporter("metrics").await.is_ok());

    assert_eq!("OK", knd.call_rest_api("").await.unwrap());

    let result = knd.call_rest_api(routes::GET_INFO).await.unwrap();
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
    let mut knd = knd!(&bitcoin, &cockroach);
    knd.start().await?;

    sleep_until(Instant::now() + Duration::from_secs(10000)).await;
    Ok(())
}
