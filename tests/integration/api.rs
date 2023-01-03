use std::thread::spawn;
use std::{fs, sync::Arc};

use futures::FutureExt;
use lightning_knd::api::start_rest_api;
use lightning_knd::api::MacaroonAuth;
use logger::KndLogger;
use once_cell::sync::Lazy;
use reqwest::RequestBuilder;
use reqwest::StatusCode;
use settings::Settings;
use test_utils::{https_client, TestSettingsBuilder};

use api::{Balance, GetInfo};
use tokio::runtime::Runtime;

use crate::MockLightning;
use crate::{quit_signal, MockWallet};

macro_rules! unauthorized {
    ($name: ident, $path: literal) => {
        #[tokio::test(flavor = "multi_thread")]
        async fn $name() {
            assert_eq!(StatusCode::UNAUTHORIZED, unauthorized_request($path).await);
        }
    };
}

unauthorized!(test_root_unauthorized, "/");
unauthorized!(test_getinfo_unauthorized, "/v1/getinfo");
unauthorized!(test_getbalance, "/v1/getbalance");

#[tokio::test(flavor = "multi_thread")]
async fn test_not_found() {
    assert_eq!(
        StatusCode::NOT_FOUND,
        admin_request("/x").await.unwrap_err()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_root_readonly() {
    assert_eq!("OK", readonly_request("/").await.unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_root_admin() {
    assert_eq!("OK", admin_request("/").await.unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_getinfo_readonly() {
    let result = readonly_request("/v1/getinfo").await.unwrap();
    let info: GetInfo = serde_json::from_str(&result).unwrap();
    assert_eq!(LIGHTNING.num_peers, info.num_peers);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_getbalance_readonly() {
    let result = readonly_request("/v1/getbalance").await.unwrap();
    let balance: Balance = serde_json::from_str(&result).unwrap();
    assert_eq!(9, balance.total_balance);
    assert_eq!(4, balance.conf_balance);
    assert_eq!(5, balance.unconf_balance);
}

static RUNTIME: Lazy<Runtime> = Lazy::new(|| Runtime::new().unwrap());

static SETTINGS: Lazy<Settings> = Lazy::new(|| {
    KndLogger::init("test", log::LevelFilter::Info);
    let settings = TestSettingsBuilder::new()
        .with_data_dir(&format!("{}/test_api", env!("CARGO_TARGET_TMPDIR")))
        .build();
    let macaroon_auth = Arc::new(MacaroonAuth::init(&[0u8; 32], &settings.data_dir).unwrap());

    let settings_clone = settings.clone();
    // Run the API with its own runtime in its own thread.
    spawn(move || {
        RUNTIME
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

static ADMIN_MACAROON: Lazy<Vec<u8>> =
    Lazy::new(|| fs::read(format!("{}/macaroons/admin_macaroon", SETTINGS.data_dir)).unwrap());

static READONLY_MACAROON: Lazy<Vec<u8>> =
    Lazy::new(|| fs::read(format!("{}/macaroons/readonly_macaroon", SETTINGS.data_dir)).unwrap());

static LIGHTNING: Lazy<Arc<MockLightning>> = Lazy::new(|| Arc::new(MockLightning::default()));

fn request_builder(path: &str) -> RequestBuilder {
    let address = &SETTINGS.rest_api_address;
    https_client().get(format!("https://{}{}", address, path))
}

async fn unauthorized_request(path: &str) -> StatusCode {
    request(request_builder(path)).await.unwrap_err()
}

async fn admin_request(path: &str) -> Result<String, StatusCode> {
    request(request_builder(path).header("macaroon", ADMIN_MACAROON.to_owned())).await
}

async fn readonly_request(path: &str) -> Result<String, StatusCode> {
    request(request_builder(path).header("macaroon", READONLY_MACAROON.to_owned())).await
}

async fn request(builder: RequestBuilder) -> Result<String, StatusCode> {
    let response = builder.send().await.unwrap();
    if !response.status().is_success() {
        return Err(response.status());
    }
    Ok(response.text().await.unwrap())
}
