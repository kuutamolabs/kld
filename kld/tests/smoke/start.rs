use std::{str::FromStr, time::Duration};

use crate::{generate_blocks, START_N_BLOCKS};
use anyhow::Result;
use api::{
    routes, Channel, ChannelState, FundChannel, FundChannelResponse, GetInfo, NewAddress,
    NewAddressResponse, WalletBalance,
};
use bitcoin::Address;
use hyper::Method;
use test_utils::{bitcoin, cockroach, kld, poll, test_settings, TEST_ADDRESS};
use tokio::time::{sleep_until, Instant};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Does not pass in CI, needs a powerful machine or BDK bug fixing"]
pub async fn test_start() -> Result<()> {
    let mut settings_0 = test_settings!("start");
    let cockroach = cockroach!(settings_0);
    let bitcoin = bitcoin!(settings_0);
    generate_blocks(
        &settings_0,
        START_N_BLOCKS,
        &Address::from_str(TEST_ADDRESS)?,
        false,
    )
    .await?;

    settings_0.node_id = "start0".to_owned();
    settings_0.database_name = "start0".to_owned();
    let kld_0 = kld!(&bitcoin, &cockroach, settings_0);

    let pid = kld_0.call_exporter("pid").await?;
    assert_eq!(pid, kld_0.pid().unwrap().to_string());
    assert!(kld_0.call_exporter("metrics").await.is_ok());

    let address: NewAddressResponse = kld_0
        .call_rest_api(Method::GET, routes::NEW_ADDR, NewAddress::default())
        .await?;

    generate_blocks(
        &settings_0,
        101, // Coinbase not spendable for 100 blocks.
        &bitcoin::Address::from_str(&address.address)?,
        false,
    )
    .await?;

    poll!(
        120,
        kld_0
            .call_rest_api::<WalletBalance, ()>(Method::GET, routes::GET_BALANCE, ())
            .await?
            .total_balance
            > 0
    );

    let mut settings_1 = settings_0.clone();
    settings_1.node_id = "start1".to_owned();
    settings_1.database_name = "start1".to_owned();
    let kld_1 = kld!(&bitcoin, &cockroach, settings_1);

    let info_1: GetInfo = kld_1
        .call_rest_api(Method::GET, routes::GET_INFO, ())
        .await?;

    let fund_channel = FundChannel {
        id: format!("{}@127.0.0.1:{}", info_1.id, kld_1.peer_port),
        satoshis: "1000000".to_string(),
        ..Default::default()
    };

    let _open_channel_response: FundChannelResponse = kld_0
        .call_rest_api(Method::POST, routes::OPEN_CHANNEL, fund_channel)
        .await?;

    generate_blocks(
        &settings_0,
        10,
        &bitcoin::Address::from_str(&address.address)?,
        true,
    )
    .await?;

    poll!(
        120,
        kld_0
            .call_rest_api::<Vec<Channel>, ()>(Method::GET, routes::LIST_CHANNELS, ())
            .await?
            .get(0)
            .map(|c| &c.state)
            == Some(&ChannelState::Usable)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Only run this for manual testing"]
pub async fn test_manual() -> Result<()> {
    let mut settings = test_settings!("manual");
    let cockroach = cockroach!(settings);
    let bitcoin = bitcoin!(settings);

    generate_blocks(
        &settings,
        START_N_BLOCKS,
        &Address::from_str(TEST_ADDRESS)?,
        false,
    )
    .await?;
    let _kld = kld!(&bitcoin, &cockroach, settings);

    sleep_until(Instant::now() + Duration::from_secs(10000)).await;
    Ok(())
}
