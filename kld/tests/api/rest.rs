use std::assert_eq;
use std::net::SocketAddr;
use std::str::FromStr;
use std::thread::spawn;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, sync::Arc};

use anyhow::{Context, Result};
use axum::http::HeaderValue;
use futures::FutureExt;
use hyper::header::CONTENT_TYPE;
use hyper::Method;
use kld::api::bind_api_server;
use kld::api::MacaroonAuth;
use kld::logger::KldLogger;
use kld::settings::Settings;
use once_cell::sync::Lazy;
use reqwest::RequestBuilder;
use reqwest::StatusCode;
use serde::Serialize;
use test_utils::ports::get_available_port;
use test_utils::{
    https_client, poll, test_settings, TEST_ADDRESS, TEST_ALIAS, TEST_PUBLIC_KEY,
    TEST_SHORT_CHANNEL_ID, TEST_TX, TEST_TX_ID,
};

use api::{
    routes, Channel, ChannelFee, ChannelState, FeeRate, FeeRatesResponse, FundChannel,
    FundChannelResponse, GenerateInvoice, GenerateInvoiceResponse, GetInfo, Invoice, InvoiceStatus,
    KeysendRequest, ListFunds, NetworkChannel, NetworkNode, NewAddress, NewAddressResponse,
    OutputStatus, PayInvoice, Payment, PaymentResponse, Peer, SetChannelFeeResponse, SignRequest,
    SignResponse, WalletBalance, WalletTransfer, WalletTransferResponse,
};
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

use crate::mocks::mock_bitcoind::MockBitcoind;
use crate::mocks::mock_lightning::MockLightning;
use crate::mocks::mock_wallet::MockWallet;
use crate::quit_signal;

#[tokio::test(flavor = "multi_thread")]
pub async fn test_unauthorized() -> Result<()> {
    let context = create_api_server().await?;
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::GET, routes::ROOT)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::POST, routes::SIGN)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        readonly_request_with_body(&context, Method::POST, routes::SIGN, || SignRequest {
            message: "testmessage".to_string()
        })?
        .send()
        .await?
        .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::GET, routes::GET_INFO)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::GET, routes::GET_BALANCE)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::GET, routes::LIST_FUNDS)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::GET, routes::LIST_CHANNELS)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::POST, routes::OPEN_CHANNEL)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        readonly_request_with_body(
            &context,
            Method::POST,
            routes::OPEN_CHANNEL,
            fund_channel_request
        )?
        .send()
        .await?
        .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::POST, routes::SET_CHANNEL_FEE,)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        readonly_request_with_body(
            &context,
            Method::POST,
            routes::SET_CHANNEL_FEE,
            set_channel_fee_request
        )?
        .send()
        .await?
        .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::DELETE, routes::CLOSE_CHANNEL,)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        readonly_request(
            &context,
            Method::DELETE,
            &routes::CLOSE_CHANNEL.replace(":id", &TEST_SHORT_CHANNEL_ID.to_string()),
        )?
        .send()
        .await?
        .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::POST, routes::WITHDRAW)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        readonly_request_with_body(&context, Method::POST, routes::WITHDRAW, withdraw_request)?
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        readonly_request_with_body(&context, Method::GET, routes::NEW_ADDR, NewAddress::default)?
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        readonly_request_with_body(&context, Method::GET, routes::NEW_ADDR, NewAddress::default)?
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::GET, routes::LIST_PEERS)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::POST, routes::CONNECT_PEER)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        readonly_request_with_body(&context, Method::POST, routes::CONNECT_PEER, || {
            TEST_ADDRESS
        })?
        .send()
        .await?
        .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::DELETE, routes::DISCONNECT_PEER)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        readonly_request(
            &context,
            Method::DELETE,
            &routes::DISCONNECT_PEER.replace(":id", TEST_PUBLIC_KEY),
        )?
        .send()
        .await?
        .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::GET, routes::LIST_NETWORK_NODE)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::GET, routes::LIST_NETWORK_NODES)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::GET, routes::LIST_NETWORK_CHANNEL)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::GET, routes::LIST_NETWORK_CHANNELS)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::GET, routes::FEE_RATES)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::POST, routes::KEYSEND)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        readonly_request_with_body(&context, Method::POST, routes::KEYSEND, keysend_request)?
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::GET, routes::LIST_INVOICES)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::POST, routes::GENERATE_INVOICE)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::POST, routes::PAY_INVOICE)
            .send()
            .await?
            .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        readonly_request_with_body(&context, Method::POST, routes::PAY_INVOICE, || PayInvoice {
            label: Some("label".to_string()),
            bolt11: LIGHTNING.invoice.bolt11.to_string()
        })?
        .send()
        .await?
        .status()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        unauthorized_request(&context, Method::GET, routes::LIST_PAYMENTS)
            .send()
            .await?
            .status()
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_not_found() -> Result<()> {
    let context = create_api_server().await?;
    assert_eq!(
        StatusCode::NOT_FOUND,
        admin_request(&context, Method::GET, "/x")?
            .send()
            .await?
            .status()
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_root_readonly() -> Result<()> {
    let context = create_api_server().await?;
    assert!(readonly_request(&context, Method::GET, routes::ROOT)?
        .send()
        .await?
        .status()
        .is_success());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_root_admin() -> Result<()> {
    let context = create_api_server().await?;
    assert!(admin_request(&context, Method::GET, routes::ROOT)?
        .send()
        .await?
        .status()
        .is_success());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sign_admin() -> Result<()> {
    let context = create_api_server().await?;
    let response: SignResponse =
        admin_request_with_body(&context, Method::POST, routes::SIGN, || SignRequest {
            message: "testmessage".to_string(),
        })?
        .send()
        .await?
        .json()
        .await?;
    assert_eq!("1234abcd", response.signature);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_info_readonly() -> Result<()> {
    let context = create_api_server().await?;
    let info: GetInfo = readonly_request(&context, Method::GET, routes::GET_INFO)?
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(info.address, vec!["127.0.0.1:2312", "[2001:db8::1]:8080"]);
    assert_eq!(LIGHTNING.num_peers, info.num_peers);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_balance_readonly() -> Result<()> {
    let context = create_api_server().await?;
    let balance: WalletBalance = readonly_request(&context, Method::GET, routes::GET_BALANCE)?
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(9, balance.total_balance);
    assert_eq!(4, balance.conf_balance);
    assert_eq!(5, balance.unconf_balance);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_funds_readonly() -> Result<()> {
    let context = create_api_server().await?;
    let funds: ListFunds = readonly_request(&context, Method::GET, routes::LIST_FUNDS)?
        .send()
        .await?
        .json()
        .await?;

    let output = funds.outputs.get(0).context("Missing output")?;
    assert_eq!(TEST_TX_ID, output.txid);
    assert_eq!(0, output.output);
    assert_eq!(546, output.value);
    assert_eq!(546000, output.amount_msat);
    assert_eq!(
        "bc1prx7399hvfe8hta6lfn2qncvczxjeur5cwlrpxhwrzqssj9kuqpeqchh5xf",
        output.address
    );
    assert_eq!(OutputStatus::Confirmed, output.status);
    assert_eq!(Some(600000), output.block_height);

    let channel = funds.channels.get(0).context("Missing channel")?;
    assert_eq!(TEST_PUBLIC_KEY, channel.peer_id);
    assert!(channel.connected);
    assert_eq!(ChannelState::Usable, channel.state);
    assert_eq!(TEST_SHORT_CHANNEL_ID.to_string(), channel.short_channel_id);
    assert_eq!(1000000, channel.channel_sat);
    assert_eq!(10001, channel.our_amount_msat);
    assert_eq!(1000000000, channel.amount_msat);
    assert_eq!(TEST_TX_ID, channel.funding_txid);
    assert_eq!(2, channel.funding_output);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_channels_readonly() -> Result<()> {
    let context = create_api_server().await?;
    let channels: Vec<Channel> = readonly_request(&context, Method::GET, routes::LIST_CHANNELS)?
        .send()
        .await?
        .json()
        .await?;
    let channel = channels.get(0).context("Missing channel")?;
    assert_eq!(TEST_PUBLIC_KEY, channel.id);
    assert!(channel.connected);
    assert_eq!(ChannelState::Usable, channel.state);
    assert_eq!(TEST_SHORT_CHANNEL_ID.to_string(), channel.short_channel_id);
    assert_eq!(TEST_TX_ID, channel.funding_txid);
    assert!(!channel.private);
    assert_eq!(10001, channel.msatoshi_to_us);
    assert_eq!(1000000000, channel.msatoshi_total);
    assert_eq!(999989999, channel.msatoshi_to_them);
    assert_eq!(5000, channel.their_channel_reserve_satoshis);
    assert_eq!(Some(10000), channel.our_channel_reserve_satoshis);
    assert_eq!(100000, channel.spendable_msatoshi);
    assert_eq!(1, channel.direction);
    assert_eq!(TEST_ALIAS, channel.alias);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_open_channel_admin() -> Result<()> {
    let context = create_api_server().await?;
    let response: FundChannelResponse = admin_request_with_body(
        &context,
        Method::POST,
        routes::OPEN_CHANNEL,
        fund_channel_request,
    )?
    .send()
    .await?
    .json()
    .await?;
    assert_eq!(TEST_TX_ID, response.txid);
    assert_eq!(
        "0101010101010101010101010101010101010101010101010101010101010101",
        response.channel_id
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_set_channel_fee_admin() -> Result<()> {
    let context = create_api_server().await?;
    let response: SetChannelFeeResponse = admin_request_with_body(
        &context,
        Method::POST,
        routes::SET_CHANNEL_FEE,
        set_channel_fee_request,
    )?
    .send()
    .await?
    .json()
    .await?;

    let fee = response.0.get(0).context("Bad response")?;
    assert_eq!(TEST_SHORT_CHANNEL_ID.to_string(), fee.short_channel_id);
    assert_eq!(TEST_PUBLIC_KEY, fee.peer_id);
    assert_eq!(set_channel_fee_request().base, Some(fee.base));
    assert_eq!(set_channel_fee_request().ppm, Some(fee.ppm));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_set_all_channel_fees_admin() -> Result<()> {
    let context = create_api_server().await?;
    let request = ChannelFee {
        id: "all".to_string(),
        base: Some(32500),
        ppm: Some(1200),
    };
    let response: SetChannelFeeResponse =
        admin_request_with_body(&context, Method::POST, routes::SET_CHANNEL_FEE, || {
            request.clone()
        })?
        .send()
        .await?
        .json()
        .await?;

    let fee = response.0.get(0).context("Bad response")?;
    assert_eq!(TEST_SHORT_CHANNEL_ID.to_string(), fee.short_channel_id);
    assert_eq!(TEST_PUBLIC_KEY, fee.peer_id);
    assert_eq!(request.base, Some(fee.base));
    assert_eq!(request.ppm, Some(fee.ppm));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_close_channel_admin() -> Result<()> {
    let context = create_api_server().await?;
    let result = admin_request(
        &context,
        Method::DELETE,
        &routes::CLOSE_CHANNEL.replace(":id", &TEST_SHORT_CHANNEL_ID.to_string()),
    )?
    .send()
    .await?;
    assert!(result.status().is_success());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_withdraw_admin() -> Result<()> {
    let context = create_api_server().await?;
    let response: WalletTransferResponse =
        admin_request_with_body(&context, Method::POST, routes::WITHDRAW, withdraw_request)?
            .send()
            .await?
            .json()
            .await?;
    assert_eq!(TEST_TX, response.tx);
    assert_eq!(TEST_TX_ID, response.txid);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_new_address_admin() -> Result<()> {
    let context = create_api_server().await?;
    let response: NewAddressResponse =
        admin_request_with_body(&context, Method::GET, routes::NEW_ADDR, NewAddress::default)?
            .send()
            .await?
            .json()
            .await?;
    assert_eq!(TEST_ADDRESS.to_string(), response.address);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_peers_readonly() -> Result<()> {
    let context = create_api_server().await?;
    let response: Vec<Peer> = readonly_request(&context, Method::GET, routes::LIST_PEERS)?
        .send()
        .await?
        .json()
        .await?;
    let socket_addr: SocketAddr = "127.0.0.1:5555".parse().unwrap();
    let netaddr = Some(socket_addr.to_string());
    assert!(response.contains(&Peer {
        id: TEST_PUBLIC_KEY.to_string(),
        connected: true,
        netaddr,
        alias: TEST_ALIAS.to_string()
    }));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_connect_peer_admin() -> Result<()> {
    let context = create_api_server().await?;
    let response: String =
        admin_request_with_body(&context, Method::POST, routes::CONNECT_PEER, || {
            TEST_PUBLIC_KEY
        })?
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(TEST_PUBLIC_KEY, response);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_disconnect_peer_admin() -> Result<()> {
    let context = create_api_server().await?;
    let response = admin_request(
        &context,
        Method::DELETE,
        &routes::DISCONNECT_PEER.replace(":id", TEST_PUBLIC_KEY),
    )?
    .send()
    .await?;
    assert!(response.status().is_success());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_disconnect_peer_admin_malformed_key() -> Result<()> {
    let context = create_api_server().await?;
    let response: api::Error = admin_request(
        &context,
        Method::DELETE,
        &routes::DISCONNECT_PEER.replace(":id", "abcd"),
    )?
    .send()
    .await?
    .json()
    .await?;
    assert_eq!(response.status, StatusCode::BAD_REQUEST.to_string());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_network_node_readonly() -> Result<()> {
    let context = create_api_server().await?;
    let nodes: Vec<NetworkNode> = readonly_request(
        &context,
        Method::GET,
        &routes::LIST_NETWORK_NODE.replace(":id", TEST_PUBLIC_KEY),
    )?
    .send()
    .await?
    .json()
    .await?;
    let node = nodes.get(0).context("no node in response")?;
    assert_eq!(TEST_PUBLIC_KEY, node.node_id);
    assert_eq!(TEST_ALIAS, node.alias);
    assert_eq!("010203", node.color);
    assert_eq!(21000000, node.last_timestamp);
    assert!(!node.features.is_empty());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_network_nodes_readonly() -> Result<()> {
    let context = create_api_server().await?;
    let nodes: Vec<NetworkNode> =
        readonly_request(&context, Method::GET, routes::LIST_NETWORK_NODES)?
            .send()
            .await?
            .json()
            .await?;
    assert_eq!(TEST_PUBLIC_KEY, nodes.get(0).context("bad result")?.node_id);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_network_channel_readonly() -> Result<()> {
    let context = create_api_server().await?;
    let response = readonly_request(
        &context,
        Method::GET,
        &routes::LIST_NETWORK_CHANNEL.replace(":id", "123456789"),
    )?
    .send()
    .await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_network_channels_readonly() -> Result<()> {
    let context = create_api_server().await?;
    let _channels: Vec<NetworkChannel> =
        readonly_request(&context, Method::GET, routes::LIST_NETWORK_CHANNELS)?
            .send()
            .await?
            .json()
            .await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_fee_rates() -> Result<()> {
    let context = create_api_server().await?;
    let fee_rates: FeeRatesResponse = readonly_request(
        &context,
        Method::GET,
        &routes::FEE_RATES.replace(":style", "perkb"),
    )?
    .send()
    .await?
    .json()
    .await?;
    let perkb = fee_rates.perkb.context("expected perkb fee rate")?;
    assert_eq!(1600000, perkb.urgent);
    assert_eq!(800000, perkb.normal);
    assert_eq!(400000, perkb.slow);
    assert_eq!(3101, perkb.min_acceptable);
    assert_eq!(1600000, perkb.max_acceptable);
    assert_eq!(
        121600,
        fee_rates.onchain_fee_estimates.opening_channel_satoshis
    );
    assert_eq!(
        104000,
        fee_rates.onchain_fee_estimates.mutual_close_satoshis
    );
    assert_eq!(
        120000,
        fee_rates.onchain_fee_estimates.unilateral_close_satoshis
    );

    let fee_rates: FeeRatesResponse = readonly_request(
        &context,
        Method::GET,
        &routes::FEE_RATES.replace(":style", "perkw"),
    )?
    .send()
    .await?
    .json()
    .await?;

    let perkw = fee_rates.perkw.context("expected perkw fee rate")?;
    assert_eq!(400000, perkw.urgent);
    assert_eq!(200000, perkw.normal);
    assert_eq!(100000, perkw.slow);
    assert_eq!(775, perkw.min_acceptable);
    assert_eq!(400000, perkw.max_acceptable);
    assert_eq!(
        121600,
        fee_rates.onchain_fee_estimates.opening_channel_satoshis
    );
    assert_eq!(
        104000,
        fee_rates.onchain_fee_estimates.mutual_close_satoshis
    );
    assert_eq!(
        120000,
        fee_rates.onchain_fee_estimates.unilateral_close_satoshis
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_generate_invoice() -> Result<()> {
    let context = create_api_server().await?;
    let expiry = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as u32;
    let invoice_request = GenerateInvoice {
        amount: 400004,
        label: "test label".to_string(),
        description: "test description".to_string(),
        expiry: Some(expiry),
        private: None,
        fallbacks: None,
        preimage: None,
    };
    let response: GenerateInvoiceResponse =
        admin_request_with_body(&context, Method::POST, routes::GENERATE_INVOICE, || {
            invoice_request.clone()
        })?
        .send()
        .await?
        .json()
        .await?;
    let bolt11 = lightning_invoice::Bolt11Invoice::from_str(&response.bolt11)?;
    assert_eq!(bolt11.payment_hash().to_string(), response.payment_hash);
    assert!(response.expires_at > expiry);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_invoice_unpaid() -> Result<()> {
    let context = create_api_server().await?;
    let invoice = &LIGHTNING.invoice;
    let response: Vec<Invoice> = admin_request(&context, Method::GET, routes::LIST_INVOICES)?
        .send()
        .await?
        .json()
        .await?;
    let invoice_response = response.get(0).context("expected invoice")?;
    assert_eq!(invoice.label, invoice_response.label);
    assert_eq!(invoice.bolt11.to_string(), invoice_response.bolt11);
    assert_eq!(
        invoice.bolt11.payment_hash().to_string(),
        invoice_response.payment_hash
    );
    assert_eq!(InvoiceStatus::Unpaid, invoice_response.status);
    assert_eq!("test invoice description", invoice_response.description);
    assert_eq!(
        invoice.bolt11.amount_milli_satoshis(),
        invoice_response.amount_msat
    );
    assert_eq!(None, invoice_response.amount_received_msat);
    assert_eq!(
        invoice.bolt11.expires_at().map(|d| d.as_secs()),
        invoice_response.expires_at
    );
    assert!(invoice_response.paid_at.is_none());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_payments() -> Result<()> {
    let context = create_api_server().await?;
    let payment = &LIGHTNING.payment;
    let response: Vec<Payment> = admin_request(&context, Method::GET, routes::LIST_PAYMENTS)?
        .send()
        .await?
        .json()
        .await?;
    let payment_response = response.get(0).context("expected payment")?;
    assert_eq!(payment.bolt11, payment_response.bolt11);
    assert_eq!(payment.status.to_string(), payment_response.status);
    assert!(payment_response.payment_preimage.is_some());
    assert_eq!(
        payment.amount.to_string(),
        payment_response.amount_sent_msat
    );
    Ok(())
}
#[tokio::test(flavor = "multi_thread")]
async fn test_pay_invoice() -> Result<()> {
    let context = create_api_server().await?;
    let invoice = &LIGHTNING.invoice.bolt11;
    let request = PayInvoice {
        label: Some("test label".to_string()),
        bolt11: invoice.to_string(),
    };
    let response: PaymentResponse =
        admin_request_with_body(&context, Method::POST, routes::PAY_INVOICE, || request)?
            .send()
            .await?
            .json()
            .await?;
    assert_eq!(TEST_PUBLIC_KEY, response.destination);
    assert_eq!(invoice.payment_hash().to_string(), response.payment_hash);
    assert_eq!(64, response.payment_preimage.len());
    assert_eq!(
        response.created_at,
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()
    );
    assert_eq!(1, response.parts);
    assert_eq!(Some(200000), response.amount_msat);
    assert_eq!(200000, response.amount_sent_msat);
    assert_eq!("succeeded", response.status);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_keysend_admin() -> Result<()> {
    let context = create_api_server().await?;
    let response: PaymentResponse =
        admin_request_with_body(&context, Method::POST, routes::KEYSEND, keysend_request)?
            .send()
            .await?
            .json()
            .await?;
    assert_eq!(TEST_PUBLIC_KEY, response.destination);
    assert_eq!(64, response.payment_hash.len());
    assert_eq!(64, response.payment_preimage.len());
    assert_eq!(0, response.created_at);
    assert_eq!(1, response.parts);
    assert_eq!(Some(1000), response.amount_msat);
    assert_eq!(1000000, response.amount_sent_msat);
    assert_eq!("succeeded", response.status);
    Ok(())
}

fn withdraw_request() -> WalletTransfer {
    WalletTransfer {
        address: TEST_ADDRESS.to_string(),
        satoshis: "all".to_string(),
        fee_rate: Some(FeeRate::PerKw(4000)),
        min_conf: Some("3".to_string()),
        utxos: vec![],
    }
}

fn fund_channel_request() -> FundChannel {
    FundChannel {
        id: TEST_PUBLIC_KEY.to_string() + "@1.2.3.4:1234",
        satoshis: "2100000".to_string(),
        fee_rate: Some(api::FeeRate::Urgent),
        announce: Some(false),
        push_msat: Some("10000".to_string()),
        close_to: None,
        request_amt: None,
        compact_lease: None,
        min_conf: Some(5),
        utxos: vec![],
    }
}

fn set_channel_fee_request() -> ChannelFee {
    ChannelFee {
        id: TEST_SHORT_CHANNEL_ID.to_string(),
        base: Some(32500),
        ppm: Some(1200),
    }
}

fn keysend_request() -> KeysendRequest {
    KeysendRequest {
        pubkey: TEST_PUBLIC_KEY.to_string(),
        amount: 1000,
        label: None,
        maxfeepercent: None,
        retry_for: None,
        maxdelay: None,
        exemptfee: None,
    }
}

static API_RUNTIME: Lazy<Runtime> = Lazy::new(|| Runtime::new().unwrap());

static TEST_CONTEXT: Lazy<RwLock<Option<Arc<TestContext>>>> = Lazy::new(|| RwLock::new(None));

pub struct TestContext {
    pub settings: Settings,
    admin_macaroon: Vec<u8>,
    readonly_macaroon: Vec<u8>,
}

pub async fn create_api_server() -> Result<Arc<TestContext>> {
    let mut context = TEST_CONTEXT.write().await;
    if context.is_some() {
        drop(context); // release lock
        return Ok(TEST_CONTEXT.read().await.as_ref().unwrap().clone());
    }
    KldLogger::init("test", log::LevelFilter::Info);
    let rest_api_port = get_available_port().context("no port available")?;
    let rest_api_address = format!("127.0.0.1:{rest_api_port}");
    let mut settings = test_settings!("api");
    settings.rest_api_address = rest_api_address.clone();
    let certs_dir = settings.certs_dir.clone();
    let macaroon_auth = Arc::new(
        MacaroonAuth::init(&[0u8; 32], &settings.data_dir)
            .context("cannot initialize macaroon auth")?,
    );
    let admin_macaroon = admin_macaroon(&settings)?;
    let readonly_macaroon = readonly_macaroon(&settings)?;

    // Run the API with its own runtime in its own thread.
    spawn(move || {
        API_RUNTIME.spawn(async {
            bind_api_server(rest_api_address, certs_dir)
                .await?
                .serve(
                    Arc::new(MockBitcoind::default()),
                    LIGHTNING.clone(),
                    Arc::new(MockWallet::default()),
                    macaroon_auth,
                    quit_signal().shared(),
                )
                .await
        })
    });

    let new_context = TestContext {
        settings,
        admin_macaroon,
        readonly_macaroon,
    };

    poll!(
        3,
        readonly_request(&new_context, Method::GET, routes::ROOT)?
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or_default()
    );

    *context = Some(Arc::new(new_context));
    drop(context); // release lock
    Ok(TEST_CONTEXT.read().await.as_ref().unwrap().clone())
}

fn admin_macaroon(settings: &Settings) -> Result<Vec<u8>> {
    let path = format!("{}/macaroons/admin.macaroon", settings.data_dir);
    fs::read(&path).with_context(|| format!("Failed to read {path}"))
}

fn readonly_macaroon(settings: &Settings) -> Result<Vec<u8>> {
    let path = format!("{}/macaroons/readonly.macaroon", settings.data_dir);
    fs::read(&path).with_context(|| format!("Failed to read {path}"))
}

pub static LIGHTNING: Lazy<Arc<MockLightning>> = Lazy::new(|| Arc::new(MockLightning::default()));

fn unauthorized_request(context: &TestContext, method: Method, route: &str) -> RequestBuilder {
    let address = &context.settings.rest_api_address;
    https_client()
        .request(method, format!("https://{address}{route}"))
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
}

fn admin_request(context: &TestContext, method: Method, route: &str) -> Result<RequestBuilder> {
    Ok(unauthorized_request(context, method, route)
        .header("macaroon", context.admin_macaroon.clone()))
}

fn admin_request_with_body<T: Serialize, F: FnOnce() -> T>(
    context: &TestContext,
    method: Method,
    route: &str,
    f: F,
) -> Result<RequestBuilder> {
    let body = serde_json::to_string(&f())?;
    Ok(admin_request(context, method, route)?.body(body))
}

fn readonly_request(context: &TestContext, method: Method, route: &str) -> Result<RequestBuilder> {
    Ok(unauthorized_request(context, method, route)
        .header("macaroon", context.readonly_macaroon.clone()))
}

fn readonly_request_with_body<T: Serialize, F: FnOnce() -> T>(
    context: &TestContext,
    method: Method,
    route: &str,
    f: F,
) -> Result<RequestBuilder> {
    let body = serde_json::to_string(&f())?;
    Ok(readonly_request(context, method, route)?.body(body))
}
