use std::thread::spawn;
use std::{fs, sync::Arc};

use axum::http::HeaderValue;
use futures::FutureExt;
use hex::ToHex;
use hyper::header::CONTENT_TYPE;
use hyper::Method;
use lightning_knd::api::start_rest_api;
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

macro_rules! unauthorized {
    ($name: ident, $func: expr) => {
        #[tokio::test(flavor = "multi_thread")]
        async fn $name() {
            assert_eq!(StatusCode::UNAUTHORIZED, send($func).await.unwrap_err());
        }
    };
}

unauthorized!(
    test_root_unauthorized,
    unauthorized_request(Method::GET, routes::ROOT)
);
unauthorized!(
    test_getinfo_unauthorized,
    unauthorized_request(Method::GET, routes::GET_INFO)
);
unauthorized!(
    test_getbalance_unauthorized,
    unauthorized_request(Method::GET, routes::GET_BALANCE)
);
unauthorized!(
    test_listchannels_unauthorized,
    unauthorized_request(Method::GET, routes::LIST_CHANNELS)
);
unauthorized!(
    test_openchannel_unauthorized,
    unauthorized_request(Method::POST, routes::OPEN_CHANNEL)
);
unauthorized!(
    test_openchannel_readonly,
    readonly_request_with_body(Method::POST, routes::OPEN_CHANNEL, fund_channel_request)
);
unauthorized!(
    test_withdraw_unauthorized,
    unauthorized_request(Method::POST, routes::WITHDRAW)
);
unauthorized!(
    test_withdraw_readonly,
    readonly_request_with_body(Method::POST, routes::WITHDRAW, withdraw_request)
);
unauthorized!(
    test_new_address_unauthorized,
    readonly_request_with_body(Method::GET, routes::NEW_ADDR, NewAddress::default)
);
unauthorized!(
    test_new_address_readonly,
    readonly_request_with_body(Method::GET, routes::NEW_ADDR, NewAddress::default)
);
unauthorized!(
    test_list_peers_unauthorized,
    unauthorized_request(Method::GET, routes::LIST_PEERS)
);
unauthorized!(
    test_connect_peer_unauthorized,
    unauthorized_request(Method::POST, routes::CONNECT_PEER)
);
unauthorized!(
    test_connect_peer_readonly,
    readonly_request_with_body(Method::POST, routes::CONNECT_PEER, || TEST_ADDRESS)
);
unauthorized!(
    test_disconnect_peer_unauthorized,
    unauthorized_request(Method::DELETE, routes::DISCONNECT_PEER)
);
unauthorized!(
    test_disconnect_peer_readonly,
    readonly_request_with_body(Method::DELETE, routes::DISCONNECT_PEER, || TEST_ADDRESS)
);

#[tokio::test(flavor = "multi_thread")]
async fn test_not_found() {
    assert_eq!(
        StatusCode::NOT_FOUND,
        send(admin_request(Method::GET, "/x")).await.unwrap_err()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_root_readonly() {
    assert_eq!(
        "OK",
        send(readonly_request(Method::GET, routes::ROOT))
            .await
            .unwrap()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_root_admin() {
    assert_eq!(
        "OK",
        send(admin_request(Method::GET, routes::ROOT))
            .await
            .unwrap()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_getinfo_readonly() {
    let info: GetInfo = send(readonly_request(Method::GET, routes::GET_INFO))
        .await
        .deserialize();
    assert_eq!(LIGHTNING.num_peers, info.num_peers);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_getbalance_readonly() {
    let balance: WalletBalance = send(readonly_request(Method::GET, routes::GET_BALANCE))
        .await
        .deserialize();
    assert_eq!(9, balance.total_balance);
    assert_eq!(4, balance.conf_balance);
    assert_eq!(5, balance.unconf_balance);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_listchannels_readonly() {
    let channels: Vec<Channel> = send(readonly_request(Method::GET, routes::LIST_CHANNELS))
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
}

#[tokio::test(flavor = "multi_thread")]
async fn test_openchannel_admin() {
    let response: FundChannelResponse = send(admin_request_with_body(
        Method::POST,
        routes::OPEN_CHANNEL,
        fund_channel_request,
    ))
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
}

#[tokio::test(flavor = "multi_thread")]
async fn test_withdraw_admin() {
    let response: WalletTransferResponse = send(admin_request_with_body(
        Method::POST,
        routes::WITHDRAW,
        withdraw_request,
    ))
    .await
    .deserialize();
    assert_eq!("{\"version\":2,\"lock_time\":0,\"input\":[{\"previous_output\":\"0f60fdd185542f2c6ea19030b0796051e7772b6026dd5ddccd7a2f93b73e6fc2:1\",\"script_sig\":\"\",\"sequence\":4294967295,\"witness\":[]},{\"previous_output\":\"0f60fdd185542f2c6ea19030b0796051e7772b6026dd5ddccd7a2f93b73e6fc2:0\",\"script_sig\":\"\",\"sequence\":4294967295,\"witness\":[]},{\"previous_output\":\"0e53ec5dfb2cb8a71fec32dc9a634a35b7e24799295ddd5278217822e0b31f57:5\",\"script_sig\":\"\",\"sequence\":4294967295,\"witness\":[]}],\"output\":[{\"value\":1000,\"script_pubkey\":\"aaee\"},{\"value\":1000,\"script_pubkey\":\"aa\"},{\"value\":800,\"script_pubkey\":\"ff\"}]}", response.tx);
    assert_eq!(
        "fba98a9a61ef62c081b31769f66a81f1640b4f94d48b550a550034cb4990eded",
        response.txid
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_new_address_admin() {
    let response: NewAddressResponse = send(admin_request_with_body(
        Method::GET,
        routes::NEW_ADDR,
        NewAddress::default,
    ))
    .await
    .deserialize();
    assert_eq!(TEST_ADDRESS.to_string(), response.address)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_peers_readonly() {
    let response: Vec<Peer> = send(readonly_request(Method::GET, routes::LIST_PEERS))
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
}

#[tokio::test(flavor = "multi_thread")]
async fn test_connect_peer_admin() {
    let response: String = send(admin_request_with_body(
        Method::POST,
        routes::CONNECT_PEER,
        || TEST_PUBLIC_KEY,
    ))
    .await
    .deserialize();
    assert_eq!(TEST_PUBLIC_KEY, response);
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

pub static API_SETTINGS: Lazy<Settings> = Lazy::new(|| {
    KndLogger::init("test", log::LevelFilter::Info);
    let settings = TestSettingsBuilder::new()
        .with_data_dir(&format!("{}/test_api", env!("CARGO_TARGET_TMPDIR")))
        .build();
    let macaroon_auth = Arc::new(MacaroonAuth::init(&[0u8; 32], &settings.data_dir).unwrap());

    let settings_clone = settings.clone();
    // Run the API with its own runtime in its own thread.
    spawn(move || {
        API_RUNTIME
            .block_on(start_rest_api(
                settings_clone.rest_api_address.clone(),
                settings_clone.certs_dir.clone(),
                LIGHTNING.clone(),
                Arc::new(MockWallet::default()),
                macaroon_auth,
                quit_signal().shared(),
            ))
            .unwrap()
    });

    settings
});

static ADMIN_MACAROON: Lazy<Vec<u8>> = Lazy::new(|| {
    fs::read(format!(
        "{}/macaroons/admin_macaroon",
        API_SETTINGS.data_dir
    ))
    .unwrap()
});

static READONLY_MACAROON: Lazy<Vec<u8>> = Lazy::new(|| {
    fs::read(format!(
        "{}/macaroons/readonly_macaroon",
        API_SETTINGS.data_dir
    ))
    .unwrap()
});

static LIGHTNING: Lazy<Arc<MockLightning>> = Lazy::new(|| Arc::new(MockLightning::default()));

fn unauthorized_request(method: Method, route: &str) -> RequestBuilder {
    let address = &API_SETTINGS.rest_api_address;
    https_client()
        .request(method, format!("https://{}{}", address, route))
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
}

fn admin_request(method: Method, route: &str) -> RequestBuilder {
    unauthorized_request(method, route).header("macaroon", ADMIN_MACAROON.to_owned())
}

fn admin_request_with_body<T: Serialize, F: FnOnce() -> T>(
    method: Method,
    route: &str,
    f: F,
) -> RequestBuilder {
    let body = serde_json::to_string(&f()).unwrap();
    admin_request(method, route).body(body)
}

fn readonly_request(method: Method, route: &str) -> RequestBuilder {
    unauthorized_request(method, route).header("macaroon", READONLY_MACAROON.to_owned())
}

fn readonly_request_with_body<T: Serialize, F: FnOnce() -> T>(
    method: Method,
    route: &str,
    f: F,
) -> RequestBuilder {
    let body = serde_json::to_string(&f()).unwrap();
    readonly_request(method, route).body(body)
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
