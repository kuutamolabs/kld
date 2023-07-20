use std::{str::FromStr, time::Duration};

use crate::START_N_BLOCKS;
use anyhow::{Context, Result};
use api::{
    routes, Channel, ChannelState, FundChannel, FundChannelResponse, GenerateInvoice,
    GenerateInvoiceResponse, GetInfo, Invoice, KeysendRequest, NewAddress, NewAddressResponse,
    PayInvoice, PaymentResponse, WalletBalance,
};
use bitcoin::Address;
use hyper::Method;
use kld::database::payment::PaymentStatus;
use test_utils::{bitcoin, cockroach, electrs, kld, poll, test_settings, TEST_ADDRESS};
use tokio::time::{sleep_until, Instant};

#[tokio::test(flavor = "multi_thread")]
pub async fn test_start() -> Result<()> {
    let mut settings_0 = test_settings!("start");
    let cockroach = cockroach!(settings_0);
    let bitcoin = bitcoin!(settings_0);
    bitcoin
        .generate_blocks(START_N_BLOCKS, &Address::from_str(TEST_ADDRESS)?, false)
        .await?;

    settings_0.node_id = "start0".to_owned();
    settings_0.database_name = "start0".to_owned();
    let electrs_0 = electrs!(&bitcoin, settings_0);
    let kld_0 = kld!(&bitcoin, &cockroach, &electrs_0, settings_0);
    let pid = kld_0.call_exporter("pid").await?;
    assert_eq!(pid, kld_0.pid().unwrap().to_string());

    let mut settings_1 = settings_0.clone();
    settings_1.node_id = "start1".to_owned();
    settings_1.database_name = "start1".to_owned();
    let electrs_1 = electrs!(&bitcoin, settings_1);
    let kld_1 = kld!(&bitcoin, &cockroach, &electrs_1, settings_1);

    let address: NewAddressResponse = kld_0
        .call_rest_api(Method::GET, routes::NEW_ADDR, NewAddress::default())
        .await?;

    bitcoin
        .generate_blocks(1, &bitcoin::Address::from_str(&address.address)?, false)
        .await?;
    bitcoin
        .generate_blocks(
            100, // Coinbase not spendable for 100 blocks.
            &Address::from_str(TEST_ADDRESS)?,
            false,
        )
        .await?;

    let balance = 5000000000;
    let channel_amount = 1000000;
    let push_amount_msat = 1000000;
    let fee_rate_kb = 1000;
    let tx_size_bytes = 153;
    let keysend_amount_msat = 20000000;
    let invoice_amount_msat = 50000000;
    let open_channel_fee = fee_rate_kb / 1000 * tx_size_bytes;
    let kld0_open_channel_expected_balance = balance - channel_amount - open_channel_fee;
    let _kld0_close_channel_expected_balance = balance
        - open_channel_fee
        - (push_amount_msat + keysend_amount_msat + invoice_amount_msat) / 1000;
    let _kld1_close_channel_expected_balance =
        push_amount_msat + keysend_amount_msat + invoice_amount_msat;

    poll!(
        60,
        kld_0
            .call_rest_api::<WalletBalance, ()>(Method::GET, routes::GET_BALANCE, ())
            .await?
            .conf_balance
            == balance
    );

    let _info_0: GetInfo = kld_0
        .call_rest_api(Method::GET, routes::GET_INFO, ())
        .await?;

    let info_1: GetInfo = kld_1
        .call_rest_api(Method::GET, routes::GET_INFO, ())
        .await?;

    let fund_channel = FundChannel {
        id: format!("{}@127.0.0.1:{}", info_1.id, kld_1.peer_port),
        satoshis: channel_amount.to_string(),
        push_msat: Some(push_amount_msat.to_string()),
        fee_rate: Some(api::FeeRate::PerKb(fee_rate_kb as u32)),
        ..Default::default()
    };

    let _open_channel_response: FundChannelResponse = kld_0
        .call_rest_api(Method::POST, routes::OPEN_CHANNEL, fund_channel)
        .await?;

    bitcoin
        .generate_blocks(10, &bitcoin::Address::from_str(TEST_ADDRESS)?, true)
        .await?;

    poll!(
        60,
        kld_0
            .call_rest_api::<WalletBalance, ()>(Method::GET, routes::GET_BALANCE, ())
            .await?
            .conf_balance
            == kld0_open_channel_expected_balance
    );

    poll!(
        60,
        kld_1
            .call_rest_api::<Vec<Channel>, ()>(Method::GET, routes::LIST_CHANNELS, ())
            .await?
            .get(0)
            .map(|c| &c.state)
            == Some(&ChannelState::Usable)
    );
    let channels = kld_1
        .call_rest_api::<Vec<Channel>, ()>(Method::GET, routes::LIST_CHANNELS, ())
        .await?;
    let channel = channels.get(0).context("expected channel")?;

    let keysend = KeysendRequest {
        pubkey: info_1.id,
        amount: keysend_amount_msat,
        ..Default::default()
    };
    let keysend_response: PaymentResponse = kld_0
        .call_rest_api(Method::POST, routes::KEYSEND, keysend)
        .await?;
    assert_eq!(
        keysend_response.status,
        PaymentStatus::Succeeded.to_string()
    );

    let generate_invoice = GenerateInvoice {
        amount: invoice_amount_msat,
        label: "label".to_string(),
        description: "description".to_string(),
        ..Default::default()
    };
    let invoice: GenerateInvoiceResponse = kld_1
        .call_rest_api(Method::POST, routes::GENERATE_INVOICE, generate_invoice)
        .await?;
    let pay_invoice = PayInvoice {
        label: Some("payment".to_string()),
        bolt11: invoice.bolt11,
    };
    let payment: PaymentResponse = kld_0
        .call_rest_api(Method::POST, routes::PAY_INVOICE, pay_invoice)
        .await?;
    assert_eq!(payment.status, PaymentStatus::Succeeded.to_string());

    let invoices: Vec<Invoice> = kld_1
        .call_rest_api(Method::GET, routes::LIST_INVOICES, ())
        .await?;
    assert_eq!(1, invoices.len());

    kld_0
        .call_rest_api(
            Method::DELETE,
            &routes::CLOSE_CHANNEL.replace(":id", &channel.short_channel_id),
            (),
        )
        .await?;

    bitcoin
        .generate_blocks(10, &Address::from_str(TEST_ADDRESS)?, true)
        .await?;

    poll!(
        60,
        kld_1
            .call_rest_api::<WalletBalance, ()>(Method::GET, routes::GET_BALANCE, ())
            .await?
            .total_balance
            > 0
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Only run this for manual testing"]
pub async fn test_manual() -> Result<()> {
    let mut settings = test_settings!("manual");
    let cockroach = cockroach!(settings);
    let bitcoin = bitcoin!(settings);
    let electrs = electrs!(&bitcoin, settings);

    bitcoin
        .generate_blocks(START_N_BLOCKS, &Address::from_str(TEST_ADDRESS)?, false)
        .await?;
    let _kld = kld!(&bitcoin, &cockroach, &electrs, settings);

    sleep_until(Instant::now() + Duration::from_secs(10000)).await;
    Ok(())
}
