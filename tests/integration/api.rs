use std::sync::RwLock;
use std::thread::spawn;
use std::{fs, sync::Arc};

use anyhow::{bail, Context, Result};
use axum::http::HeaderValue;
use futures::FutureExt;
use hex::ToHex;
use hyper::header::CONTENT_TYPE;
use hyper::Method;
use lightning_knd::api::bind_api_server;
use lightning_knd::api::MacaroonAuth;
use logger::KndLogger;
use once_cell::sync::Lazy;
use reqwest::RequestBuilder;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::Serialize;
use settings::Settings;
use test_utils::{https_client, random_public_key, TestSettingsBuilder};

use api::{
    routes, Channel, FundChannel, FundChannelResponse, GetInfo, NewAddress, NewAddressResponse,
    Peer, WalletBalance, WalletTransfer, WalletTransferResponse,
};
use tokio::runtime::Runtime;

use crate::mocks::mock_lightning::MockLightning;
use crate::mocks::mock_wallet::MockWallet;
use crate::mocks::{TEST_ADDRESS, TEST_PUBLIC_KEY};
use crate::quit_signal;

#[tokio::test(flavor = "multi_thread")]
pub async fn test_unauthorized() -> Result<()> {
    let settings = create_api_server().await?;
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        send(unauthorized_request(&settings, Method::GET, routes::ROOT))
            .await
            .unwrap_err()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        send(unauthorized_request(
            &settings,
            Method::GET,
            routes::GET_INFO
        ))
        .await
        .unwrap_err()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        send(unauthorized_request(
            &settings,
            Method::GET,
            routes::GET_BALANCE
        ))
        .await
        .unwrap_err()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        send(unauthorized_request(
            &settings,
            Method::GET,
            routes::LIST_CHANNELS
        ))
        .await
        .unwrap_err()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        send(unauthorized_request(
            &settings,
            Method::POST,
            routes::OPEN_CHANNEL
        ))
        .await
        .unwrap_err()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        send(readonly_request_with_body(
            &settings,
            Method::POST,
            routes::OPEN_CHANNEL,
            fund_channel_request
        )?)
        .await
        .unwrap_err()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        send(unauthorized_request(
            &settings,
            Method::POST,
            routes::WITHDRAW
        ))
        .await
        .unwrap_err()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        send(readonly_request_with_body(
            &settings,
            Method::POST,
            routes::WITHDRAW,
            withdraw_request
        )?)
        .await
        .unwrap_err()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        send(readonly_request_with_body(
            &settings,
            Method::GET,
            routes::NEW_ADDR,
            NewAddress::default
        )?)
        .await
        .unwrap_err()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        send(readonly_request_with_body(
            &settings,
            Method::GET,
            routes::NEW_ADDR,
            NewAddress::default
        )?)
        .await
        .unwrap_err()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        send(unauthorized_request(
            &settings,
            Method::GET,
            routes::LIST_PEERS
        ))
        .await
        .unwrap_err()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        send(unauthorized_request(
            &settings,
            Method::POST,
            routes::CONNECT_PEER
        ))
        .await
        .unwrap_err()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        send(readonly_request_with_body(
            &settings,
            Method::POST,
            routes::CONNECT_PEER,
            || TEST_ADDRESS
        )?)
        .await
        .unwrap_err()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        send(unauthorized_request(
            &settings,
            Method::DELETE,
            routes::DISCONNECT_PEER
        ))
        .await
        .unwrap_err()
    );
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        send(readonly_request_with_body(
            &settings,
            Method::DELETE,
            routes::DISCONNECT_PEER,
            || TEST_ADDRESS
        )?)
        .await
        .unwrap_err()
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_not_found() -> Result<()> {
    let settings = create_api_server().await?;
    assert_eq!(
        StatusCode::NOT_FOUND,
        send(admin_request(&settings, Method::GET, "/x")?)
            .await
            .unwrap_err()
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_root_readonly() -> Result<()> {
    let settings = create_api_server().await?;
    assert_eq!(
        "OK",
        send(readonly_request(&settings, Method::GET, routes::ROOT)?)
            .await
            .unwrap()
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_root_admin() -> Result<()> {
    let settings = create_api_server().await?;
    assert_eq!(
        "OK",
        send(admin_request(&settings, Method::GET, routes::ROOT)?)
            .await
            .unwrap()
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_getinfo_readonly() -> Result<()> {
    let settings = create_api_server().await?;
    let info: GetInfo = send(readonly_request(&settings, Method::GET, routes::GET_INFO)?)
        .await
        .deserialize();
    assert_eq!(LIGHTNING.num_peers, info.num_peers);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_getbalance_readonly() -> Result<()> {
    let settings = create_api_server().await?;
    let balance: WalletBalance = send(readonly_request(
        &settings,
        Method::GET,
        routes::GET_BALANCE,
    )?)
    .await
    .deserialize();
    assert_eq!(9, balance.total_balance);
    assert_eq!(4, balance.conf_balance);
    assert_eq!(5, balance.unconf_balance);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_listchannels_readonly() -> Result<()> {
    let settings = create_api_server().await?;
    let channels: Vec<Channel> = send(readonly_request(
        &settings,
        Method::GET,
        routes::LIST_CHANNELS,
    )?)
    .await
    .deserialize();
    let channel = channels.get(0).unwrap();
    assert_eq!(
        "0202755b475334bd9a56a317fd23dfe264b193bcbd7322faa3e974031704068266",
        channel.id
    );
    assert_eq!("true", channel.connected);
    assert_eq!("usable", channel.state);
    assert_eq!("34234124", channel.short_channel_id);
    assert_eq!(
        "0000000000000000000000000000000000000000000000000000000000000000",
        channel.funding_txid
    );
    assert_eq!("false", channel.private);
    assert_eq!("", channel.msatoshi_to_us);
    assert_eq!("1000000", channel.msatoshi_total);
    assert_eq!("", channel.msatoshi_to_them);
    assert_eq!("5000", channel.their_channel_reserve_satoshis);
    assert_eq!("10000", channel.our_channel_reserve_satoshis);
    assert_eq!("100000", channel.spendable_msatoshi);
    assert_eq!(1, channel.direction);
    assert_eq!("test_node", channel.alias);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_openchannel_admin() -> Result<()> {
    let settings = create_api_server().await?;
    let response: FundChannelResponse = send(admin_request_with_body(
        &settings,
        Method::POST,
        routes::OPEN_CHANNEL,
        fund_channel_request,
    )?)
    .await
    .deserialize();
    assert_eq!(
        "fba98a9a61ef62c081b31769f66a81f1640b4f94d48b550a550034cb4990eded",
        response.txid
    );
    assert_eq!(
        "0101010101010101010101010101010101010101010101010101010101010101",
        response.channel_id
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_withdraw_admin() -> Result<()> {
    let settings = create_api_server().await?;
    let response: WalletTransferResponse = send(admin_request_with_body(
        &settings,
        Method::POST,
        routes::WITHDRAW,
        withdraw_request,
    )?)
    .await
    .deserialize();
    assert_eq!("{\"version\":2,\"lock_time\":0,\"input\":[{\"previous_output\":\"0f60fdd185542f2c6ea19030b0796051e7772b6026dd5ddccd7a2f93b73e6fc2:1\",\"script_sig\":\"\",\"sequence\":4294967295,\"witness\":[]},{\"previous_output\":\"0f60fdd185542f2c6ea19030b0796051e7772b6026dd5ddccd7a2f93b73e6fc2:0\",\"script_sig\":\"\",\"sequence\":4294967295,\"witness\":[]},{\"previous_output\":\"0e53ec5dfb2cb8a71fec32dc9a634a35b7e24799295ddd5278217822e0b31f57:5\",\"script_sig\":\"\",\"sequence\":4294967295,\"witness\":[]}],\"output\":[{\"value\":1000,\"script_pubkey\":\"aaee\"},{\"value\":1000,\"script_pubkey\":\"aa\"},{\"value\":800,\"script_pubkey\":\"ff\"}]}", response.tx);
    assert_eq!(
        "fba98a9a61ef62c081b31769f66a81f1640b4f94d48b550a550034cb4990eded",
        response.txid
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_new_address_admin() -> Result<()> {
    let settings = create_api_server().await?;
    let response: NewAddressResponse = send(admin_request_with_body(
        &settings,
        Method::GET,
        routes::NEW_ADDR,
        NewAddress::default,
    )?)
    .await
    .deserialize();
    assert_eq!(TEST_ADDRESS.to_string(), response.address);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_peers_readonly() -> Result<()> {
    let settings = create_api_server().await?;
    let response: Vec<Peer> = send(readonly_request(
        &settings,
        Method::GET,
        routes::LIST_PEERS,
    )?)
    .await
    .deserialize();
    let peer = response.get(0).unwrap();
    assert_eq!(
        "0202755b475334bd9a56a317fd23dfe264b193bcbd7322faa3e974031704068266",
        peer.id
    );
    assert_eq!("127.0.0.1:8080", peer.netaddr);
    assert_eq!("connected", peer.connected);
    assert_eq!("test", peer.alias);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_connect_peer_admin() -> Result<()> {
    let settings = create_api_server().await?;
    let response: String = send(admin_request_with_body(
        &settings,
        Method::POST,
        routes::CONNECT_PEER,
        || TEST_PUBLIC_KEY,
    )?)
    .await
    .deserialize();
    assert_eq!(TEST_PUBLIC_KEY, response);
    Ok(())
}

fn withdraw_request() -> WalletTransfer {
    WalletTransfer {
        address: TEST_ADDRESS.to_string(),
        satoshis: "all".to_string(),
        fee_rate: None,
        min_conf: Some("3".to_string()),
        utxos: vec![],
    }
}

fn fund_channel_request() -> FundChannel {
    FundChannel {
        id: random_public_key().serialize().encode_hex(),
        satoshis: "21000000".to_string(),
        fee_rate: Some("4".to_string()),
        announce: Some("true".to_string()),
        push_msat: Some("10000".to_string()),
        close_to: None,
        request_amt: None,
        compact_lease: None,
        min_conf: Some(5),
        utxos: vec![],
    }
}

static API_RUNTIME: Lazy<Runtime> = Lazy::new(|| Runtime::new().unwrap());

static API_SETTINGS: RwLock<Option<Settings>> = RwLock::new(None);

pub async fn create_api_server() -> Result<Settings> {
    let mut settings = match API_SETTINGS.write() {
        Ok(s) => s,
        Err(e) => bail!("failed to lock API_SETTINGS singleton: {}", e),
    };
    if settings.is_some() {
        drop(settings); // release lock
        return Ok(API_SETTINGS
            .read()
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .clone());
    }
    KndLogger::init("test", log::LevelFilter::Info);
    let s = TestSettingsBuilder::new()
        .with_data_dir(&format!("{}/test_api", env!("CARGO_TARGET_TMPDIR")))
        .build();
    let rest_api_address = s.rest_api_address.clone();
    let certs_dir = s.certs_dir.clone();

    let macaroon_auth = Arc::new(
        MacaroonAuth::init(&[0u8; 32], &s.data_dir).context("cannot initialize macaroon auth")?,
    );

    let server = bind_api_server(rest_api_address, certs_dir).await?;

    // Run the API with its own runtime in its own thread.
    spawn(move || {
        API_RUNTIME
            .block_on(server.serve(
                LIGHTNING.clone(),
                Arc::new(MockWallet::default()),
                macaroon_auth,
                quit_signal().shared(),
            ))
            .unwrap()
    });

    *settings = Some(s);
    drop(settings); // release lock
    Ok(API_SETTINGS
        .read()
        .as_ref()
        .unwrap()
        .as_ref()
        .unwrap()
        .clone())
}

// TODO: those should be read only once when parsing settings...
pub fn admin_macaroon(settings: &Settings) -> Result<Vec<u8>> {
    let path = format!("{}/macaroons/admin.macaroon", settings.data_dir);
    fs::read(&path).with_context(|| format!("Failed to read {}", path))
}

pub fn readonly_macaroon(settings: &Settings) -> Result<Vec<u8>> {
    let path = format!("{}/macaroons/readonly.macaroon", settings.data_dir);
    fs::read(&path).with_context(|| format!("Failed to read {}", path))
}

static LIGHTNING: Lazy<Arc<MockLightning>> = Lazy::new(|| Arc::new(MockLightning::default()));

fn unauthorized_request(settings: &Settings, method: Method, route: &str) -> RequestBuilder {
    let address = &settings.rest_api_address;
    https_client()
        .request(method, format!("https://{}{}", address, route))
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
}

fn admin_request(settings: &Settings, method: Method, route: &str) -> Result<RequestBuilder> {
    Ok(
        unauthorized_request(settings, method, route)
            .header("macaroon", admin_macaroon(&settings)?),
    )
}

fn admin_request_with_body<T: Serialize, F: FnOnce() -> T>(
    settings: &Settings,
    method: Method,
    route: &str,
    f: F,
) -> Result<RequestBuilder> {
    let body = serde_json::to_string(&f()).unwrap();
    Ok(admin_request(settings, method, route)?.body(body))
}

fn readonly_request(settings: &Settings, method: Method, route: &str) -> Result<RequestBuilder> {
    Ok(unauthorized_request(settings, method, route)
        .header("macaroon", readonly_macaroon(&settings)?))
}

fn readonly_request_with_body<T: Serialize, F: FnOnce() -> T>(
    settings: &Settings,
    method: Method,
    route: &str,
    f: F,
) -> Result<RequestBuilder> {
    let body = serde_json::to_string(&f()).unwrap();
    Ok(readonly_request(settings, method, route)?.body(body))
}

struct ApiResult(Result<String, StatusCode>);

impl ApiResult {
    fn deserialize<T: DeserializeOwned>(self) -> T {
        serde_json::from_str::<T>(&self.0.unwrap()).unwrap()
    }

    fn unwrap(self) -> String {
        self.0.unwrap()
    }

    fn unwrap_err(self) -> StatusCode {
        self.0.unwrap_err()
    }
}

async fn send(builder: RequestBuilder) -> ApiResult {
    let response = builder.send().await.unwrap();
    if !response.status().is_success() {
        return ApiResult(Err(response.status()));
    }
    ApiResult(Ok(response.text().await.unwrap()))
}
