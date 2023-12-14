use std::assert_eq;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::OnceLock;
use std::thread::spawn;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, sync::Arc};

use anyhow::{Context, Result};
use bitcoin::hashes::hex::ToHex;
use futures::FutureExt;
use hyper::Method;
use kld::api::bind_api_server;
use kld::api::codegen::get_v1_channel_history_response::GetV1ChannelHistoryResponseItem;
use kld::api::codegen::get_v1_channel_list_forwards_response::GetV1ChannelListForwardsResponseItem;
use kld::api::codegen::get_v1_channel_list_peer_channels_response::{
    GetV1ChannelListPeerChannelsResponse, GetV1ChannelListPeerChannelsResponseState,
};
use kld::api::codegen::get_v1_channel_localremotebal_response::GetV1ChannelLocalremotebalResponse;
use kld::api::codegen::get_v1_estimate_channel_liquidity_body::GetV1EstimateChannelLiquidityBody;
use kld::api::codegen::get_v1_estimate_channel_liquidity_response::GetV1EstimateChannelLiquidityResponse;
use kld::api::codegen::get_v1_get_fees_response::GetV1GetFeesResponse;
use kld::api::codegen::get_v1_newaddr_response::GetV1NewaddrResponse;
use kld::api::codegen::get_v1_pay_list_payments_response::{
    GetV1PayListPaymentsResponse, GetV1PayListPaymentsResponsePaymentsItemStatus,
};
use kld::api::codegen::get_v1_utility_decode_invoice_string_response::{
    GetV1UtilityDecodeInvoiceStringResponse, GetV1UtilityDecodeInvoiceStringResponseType,
};
use kld::api::codegen::post_v1_peer_connect_body::PostV1PeerConnectBody;
use kld::api::codegen::post_v1_peer_connect_response::PostV1PeerConnectResponse;
use kld::api::MacaroonAuth;
use kld::database::payment::PaymentStatus;
use kld::logger::KldLogger;
use kld::settings::Settings;
use lightning::events::ClosureReason;
use reqwest::RequestBuilder;
use reqwest::StatusCode;
use serde::Serialize;
use test_utils::ports::get_available_port;
use test_utils::{
    https_client, poll, test_settings, TempDir, TEST_ADDRESS, TEST_ALIAS, TEST_PUBLIC_KEY,
    TEST_SHORT_CHANNEL_ID, TEST_TX, TEST_TX_ID,
};

use kld::api::payloads::{
    ChannelFee, ChannelState, FeeRate, FeeRatesResponse, FundChannel, FundChannelResponse,
    GenerateInvoice, GenerateInvoiceResponse, GetInfo, Invoice, InvoiceStatus, KeysendRequest,
    ListFunds, NetworkChannel, NetworkNode, OutputStatus, PayInvoice, PaymentResponse, Peer,
    SetChannelFeeResponse, SignRequest, SignResponse, WalletBalance, WalletTransfer,
    WalletTransferResponse,
};
use kld::api::routes;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

use crate::mocks::mock_bitcoind::MockBitcoind;
use crate::mocks::mock_lightning::MockLightning;
use crate::mocks::mock_wallet::MockWallet;
use crate::quit_signal;

#[tokio::test(flavor = "multi_thread")]
pub async fn test_unauthorized() -> Result<()> {
    let context = create_api_server().await?;
    let admin_functions = vec![
        (Method::POST, routes::SIGN),
        (Method::POST, routes::OPEN_CHANNEL),
        (Method::POST, routes::SET_CHANNEL_FEE),
        (Method::DELETE, routes::CLOSE_CHANNEL),
        (Method::DELETE, routes::FORCE_CLOSE_CHANNEL_WITH_BROADCAST),
        (
            Method::DELETE,
            routes::FORCE_CLOSE_CHANNEL_WITHOUT_BROADCAST,
        ),
        (Method::POST, routes::WITHDRAW),
        (Method::GET, routes::NEW_ADDR),
        (Method::POST, routes::CONNECT_PEER),
        (Method::DELETE, routes::DISCONNECT_PEER),
        (Method::POST, routes::KEYSEND),
        (Method::POST, routes::GENERATE_INVOICE),
        (Method::POST, routes::PAY_INVOICE),
    ];
    for (method, route) in &admin_functions {
        assert_eq!(
            StatusCode::UNAUTHORIZED,
            readonly_request_with_body(&context, method.clone(), route, || ())?
                .send()
                .await?
                .status()
        );
    }
    let mut readonly_functions = vec![
        (Method::GET, routes::ROOT),
        (Method::GET, routes::GET_INFO),
        (Method::GET, routes::GET_BALANCE),
        (Method::GET, routes::LIST_FUNDS),
        (Method::GET, routes::LIST_PEERS),
        (Method::GET, routes::LIST_NETWORK_NODE),
        (Method::GET, routes::LIST_NETWORK_NODES),
        (Method::GET, routes::LIST_NETWORK_CHANNEL),
        (Method::GET, routes::LIST_NETWORK_CHANNELS),
        (Method::GET, routes::FEE_RATES),
        (Method::GET, routes::LIST_INVOICES),
        (Method::GET, routes::LIST_PAYMENTS),
        (Method::GET, routes::ESTIMATE_CHANNEL_LIQUIDITY),
        (Method::GET, routes::LOCAL_REMOTE_BALANCE),
        (Method::GET, routes::GET_FEES),
        (Method::GET, routes::LIST_FORWARDS),
        (Method::GET, routes::LIST_CHANNEL_HISTORY),
        (Method::GET, routes::LIST_PEER_CHANNELS),
        (Method::GET, routes::DECODE_INVOICE),
    ];
    readonly_functions.extend(admin_functions.into_iter());
    for (method, route) in readonly_functions {
        assert_eq!(
            StatusCode::UNAUTHORIZED,
            unauthorized_request(&context, method, route)
                .unwrap()
                .send()
                .await?
                .status()
        );
    }
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
    assert_eq!(mock_lightning().num_peers, info.num_peers);
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
    assert_eq!(546000, output.amount_msat);
    assert_eq!(
        "bc1prx7399hvfe8hta6lfn2qncvczxjeur5cwlrpxhwrzqssj9kuqpeqchh5xf",
        output.address
    );
    assert_eq!(93, output.scriptpubkey.len());
    assert_eq!(OutputStatus::Confirmed, output.status);
    assert_eq!(Some(600000), output.block_height);

    let channel = funds.channels.get(0).context("Missing channel")?;
    assert_eq!(TEST_PUBLIC_KEY, channel.peer_id);
    assert!(channel.connected);
    assert_eq!(ChannelState::Usable, channel.state);
    assert_eq!(TEST_SHORT_CHANNEL_ID.to_string(), channel.short_channel_id);
    assert_eq!(1000000, channel.channel_sat);
    assert_eq!(100000, channel.our_amount_msat);
    assert_eq!(1000000000, channel.amount_msat);
    assert_eq!(TEST_TX_ID, channel.funding_txid);
    assert_eq!(2, channel.funding_output);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_peer_channels_readonly() -> Result<()> {
    let context = create_api_server().await?;
    let channels: Vec<GetV1ChannelListPeerChannelsResponse> =
        readonly_request(&context, Method::GET, routes::LIST_PEER_CHANNELS)?
            .send()
            .await?
            .json()
            .await?;
    let channel = channels.get(0).context("Missing channel")?;
    assert_eq!(TEST_PUBLIC_KEY, channel.peer_id);
    assert!(channel.peer_connected);
    assert_eq!(
        Some(TEST_SHORT_CHANNEL_ID.to_string()),
        channel.short_channel_id
    );
    assert_eq!(Some(TEST_TX_ID.to_string()), channel.funding_txid);
    assert!(!channel.private);
    assert!(matches!(
        channel.state,
        GetV1ChannelListPeerChannelsResponseState::ChanneldNormal
    ));
    assert_eq!(100000, channel.to_us_msat);
    assert_eq!(1000000000, channel.total_msat);
    assert_eq!(999900000, channel.to_them_msat);
    assert_eq!(5000000, channel.their_reserve_msat);
    assert_eq!(Some(10000000), channel.our_reserve_msat);
    assert_eq!(100000, channel.spendable_msat);
    assert_eq!(TEST_ALIAS, channel.alias);
    assert_eq!(5000, channel.dust_limit_msat);
    assert_eq!(
        vec![
            "supported SCIDPrivacy".to_string(),
            "required ZeroConf".to_string()
        ],
        channel.features
    );
    assert_eq!(1000, channel.fee_base_msat);
    assert_eq!(0, channel.fee_proportional_millionths);
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
async fn test_force_close_channel_with_broadcast_admin() -> Result<()> {
    let context = create_api_server().await?;
    let result = admin_request(
        &context,
        Method::DELETE,
        &routes::FORCE_CLOSE_CHANNEL_WITH_BROADCAST
            .replace(":id", &TEST_SHORT_CHANNEL_ID.to_string()),
    )?
    .send()
    .await?;
    assert!(result.status().is_success());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_force_close_channel_without_broadcast_admin() -> Result<()> {
    let context = create_api_server().await?;
    let result = admin_request(
        &context,
        Method::DELETE,
        &routes::FORCE_CLOSE_CHANNEL_WITHOUT_BROADCAST
            .replace(":id", &TEST_SHORT_CHANNEL_ID.to_string()),
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
    let response: GetV1NewaddrResponse = admin_request(&context, Method::GET, routes::NEW_ADDR)?
        .query(&[("addressType", "bech32")])
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
    let response: PostV1PeerConnectResponse =
        admin_request_with_body(&context, Method::POST, routes::CONNECT_PEER, || {
            PostV1PeerConnectBody {
                id: format!("{}@1.0.0.0:1111", TEST_PUBLIC_KEY),
            }
        })?
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(TEST_PUBLIC_KEY, response.id);
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
    let response: kld::api::payloads::Error = admin_request(
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
    let invoice = &mock_lightning().invoice;
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
    let payment = &mock_lightning().payment;
    let response: GetV1PayListPaymentsResponse = admin_request(
        &context,
        Method::GET,
        &format!("{}?direction={}", routes::LIST_PAYMENTS, payment.direction),
    )?
    .send()
    .await?
    .json()
    .await?;
    let payment_response = response.payments.get(0).context("expected payment")?;
    assert_eq!(payment.id.0.to_hex(), payment_response.id);
    assert_eq!(
        payment.bolt11.as_ref().map(|b| b.to_string()),
        payment_response.bolt11
    );
    assert!(matches!(
        payment_response.status,
        GetV1PayListPaymentsResponsePaymentsItemStatus::Pending
    ));
    assert!(payment_response.payment_preimage.is_none());
    assert_eq!(payment.amount, payment_response.amount_sent_msat);
    Ok(())
}
#[tokio::test(flavor = "multi_thread")]
async fn test_pay_invoice() -> Result<()> {
    let context = create_api_server().await?;
    let invoice = &mock_lightning().invoice.bolt11;
    let request = PayInvoice {
        label: Some("test label".to_string()),
        invoice: invoice.to_string(),
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
    assert!(response.payment_preimage.is_empty());
    assert!(response.created_at > 0);
    assert_eq!(1, response.parts);
    assert_eq!(Some(1000), response.amount_msat);
    assert_eq!(1000000, response.amount_sent_msat);
    assert_eq!(PaymentStatus::Pending.to_string(), response.status);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_estimate_liquidity() -> Result<()> {
    let context = create_api_server().await?;
    let response: GetV1EstimateChannelLiquidityResponse = readonly_request_with_body(
        &context,
        Method::GET,
        routes::ESTIMATE_CHANNEL_LIQUIDITY,
        || GetV1EstimateChannelLiquidityBody {
            scid: TEST_SHORT_CHANNEL_ID,
            target: TEST_PUBLIC_KEY.to_string(),
        },
    )?
    .send()
    .await?
    .json()
    .await?;
    assert_eq!(100, response.minimum);
    assert_eq!(100000, response.maximum);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_local_remote_balance() -> Result<()> {
    let context = create_api_server().await?;
    let response: GetV1ChannelLocalremotebalResponse =
        readonly_request(&context, Method::GET, routes::LOCAL_REMOTE_BALANCE)?
            .send()
            .await?
            .json()
            .await?;
    assert_eq!(0, response.inactive_balance);
    assert_eq!(0, response.pending_balance);
    assert_eq!(100, response.local_balance);
    assert_eq!(999900, response.remote_balance);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_fees() -> Result<()> {
    let context = create_api_server().await?;
    let response: GetV1GetFeesResponse = readonly_request(&context, Method::GET, routes::GET_FEES)?
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(3000, response.fee_collected);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_fetch_forwards() -> Result<()> {
    let context = create_api_server().await?;
    let response: Vec<GetV1ChannelListForwardsResponseItem> =
        readonly_request(&context, Method::GET, routes::LIST_FORWARDS)?
            .send()
            .await?
            .json()
            .await?;
    assert_eq!(1, response.len());
    let forward: &GetV1ChannelListForwardsResponseItem =
        response.first().context("expected forward")?;
    assert_eq!(Some(5000000), forward.in_msat);
    assert_eq!(Some(3000), forward.fee_msat);
    assert_eq!(
        mock_lightning().forward.inbound_channel_id.to_hex(),
        forward.in_channel
    );
    assert_eq!(
        mock_lightning()
            .forward
            .outbound_channel_id
            .map(|x| x.to_hex()),
        forward.out_channel
    );
    assert_eq!(Some(4997000), forward.out_msat);
    assert_eq!(None, forward.payment_hash);
    assert!(forward.received_timestamp > 0);
    assert!(forward.resolved_timestamp.is_some());
    assert_eq!(None, forward.failcode);
    assert_eq!(None, forward.failreason);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_channel_history() -> Result<()> {
    let context = create_api_server().await?;
    let response: Vec<GetV1ChannelHistoryResponseItem> =
        readonly_request(&context, Method::GET, routes::LIST_CHANNEL_HISTORY)?
            .send()
            .await?
            .json()
            .await?;
    let channel = response.first().context("expected channel")?;
    assert_eq!(mock_lightning().channel.channel_id.to_hex(), channel.id);
    assert_eq!(mock_lightning().channel.channel_id.to_hex(), channel.id);
    assert_eq!(TEST_SHORT_CHANNEL_ID, channel.scid);
    assert_eq!(
        mock_lightning().channel.user_channel_id,
        channel.user_channel_id
    );
    assert_eq!(TEST_PUBLIC_KEY, channel.counterparty);
    assert_eq!(format!("{TEST_TX_ID}:2"), channel.funding_txo);
    assert!(channel.is_public);
    assert!(channel.is_outbound);
    assert!(channel.open_timestamp > 0);
    assert!(channel.close_timestamp > 0);
    assert_eq!(
        channel.closure_reason,
        ClosureReason::CooperativeClosure.to_string()
    );
    assert_eq!(channel.value, 1000000);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_decode_invoice() -> Result<()> {
    let context = create_api_server().await?;
    let invoice = &mock_lightning().invoice.bolt11;
    let response: GetV1UtilityDecodeInvoiceStringResponse = readonly_request(
        &context,
        Method::GET,
        &routes::DECODE_INVOICE.replace(":invoice", &invoice.to_string()),
    )?
    .send()
    .await?
    .json()
    .await?;
    assert!(matches!(
        response.type_,
        GetV1UtilityDecodeInvoiceStringResponseType::Bolt11
    ));
    assert!(response.valid);
    assert_eq!(response.expiry, Some(2322));
    assert_eq!(response.currency, Some("bcrt".to_string()));
    assert_eq!(response.amount_msat, Some(200000));
    assert_eq!(response.payee, Some(TEST_PUBLIC_KEY.to_string()));
    assert_eq!(response.min_final_cltv_expiry, Some(144));
    assert!(response.created_at.is_some());
    assert!(response.payment_hash.is_some());
    assert!(response.signature.is_some());
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
        fee_rate: Some(kld::api::payloads::FeeRate::Urgent),
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

static API_RUNTIME: OnceLock<Runtime> = OnceLock::new();

static TEST_CONTEXT: OnceLock<RwLock<Option<Arc<TestContext>>>> = OnceLock::new();

pub static LIGHTNING: OnceLock<Arc<MockLightning>> = OnceLock::new();

pub fn mock_lightning() -> Arc<MockLightning> {
    LIGHTNING
        .get_or_init(|| Arc::new(MockLightning::default()))
        .clone()
}

pub struct TestContext {
    pub settings: Settings,
    admin_macaroon: Vec<u8>,
    readonly_macaroon: Vec<u8>,
    _tmp_dir: TempDir,
}

pub async fn create_api_server() -> Result<Arc<TestContext>> {
    let mut context = TEST_CONTEXT.get_or_init(|| RwLock::new(None)).write().await;
    if context.is_some() {
        drop(context); // release lock
        return Ok(TEST_CONTEXT
            .get()
            .unwrap()
            .read()
            .await
            .as_ref()
            .unwrap()
            .clone());
    }
    KldLogger::init("test", log::LevelFilter::Info);
    let tmp_dir = TempDir::new()?;
    let rest_api_port = get_available_port().context("no port available")?;
    let rest_api_address = format!("127.0.0.1:{rest_api_port}");
    let mut settings = test_settings(&tmp_dir, "api");
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
        API_RUNTIME
            .get_or_init(|| Runtime::new().unwrap())
            .spawn(async {
                bind_api_server(rest_api_address, certs_dir)
                    .await?
                    .serve(
                        Arc::new(MockBitcoind),
                        mock_lightning(),
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
        _tmp_dir: tmp_dir,
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
    Ok(TEST_CONTEXT
        .get()
        .unwrap()
        .read()
        .await
        .as_ref()
        .unwrap()
        .clone())
}

fn admin_macaroon(settings: &Settings) -> Result<Vec<u8>> {
    let path = format!("{}/macaroons/admin.macaroon", settings.data_dir);
    fs::read(&path).with_context(|| format!("Failed to read {path}"))
}

fn readonly_macaroon(settings: &Settings) -> Result<Vec<u8>> {
    let path = format!("{}/macaroons/readonly.macaroon", settings.data_dir);
    fs::read(&path).with_context(|| format!("Failed to read {path}"))
}

fn unauthorized_request(
    context: &TestContext,
    method: Method,
    route: &str,
) -> Result<RequestBuilder> {
    let address = &context.settings.rest_api_address;
    Ok(https_client(None)?.request(method, format!("https://{address}{route}")))
}

fn admin_request(context: &TestContext, method: Method, route: &str) -> Result<RequestBuilder> {
    let address = &context.settings.rest_api_address;
    Ok(https_client(Some(context.admin_macaroon.clone()))?
        .request(method, format!("https://{address}{route}")))
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
    let address = &context.settings.rest_api_address;
    Ok(https_client(Some(context.readonly_macaroon.clone()))?
        .request(method, format!("https://{address}{route}")))
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
