use std::{fs, sync::Arc};

use futures::FutureExt;
use lightning_knd::api::start_rest_api;
use lightning_knd::api::MacaroonAuth;
use reqwest::StatusCode;
use test_utils::test_settings;

use api::GetInfo;

use crate::quit_signal;
use crate::MockLightning;

#[tokio::test(flavor = "multi_thread")]
async fn test_api() {
    let mut settings = test_settings();
    settings.data_dir = format!("{}/test_api", env!("CARGO_TARGET_TMPDIR"));

    let mock_lightning = Arc::new(MockLightning::default());
    let macaroon_auth = Arc::new(MacaroonAuth::init(&[0u8; 32], &settings.data_dir).unwrap());

    tokio::spawn(start_rest_api(
        settings.rest_api_address.clone(),
        mock_lightning.clone(),
        macaroon_auth,
        quit_signal().shared(),
    ));
    let result = reqwest::Client::new()
        .get(format!("http://{}/", settings.rest_api_address))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!("OK", result);

    let status = reqwest::Client::new()
        .get(format!("http://{}/v1/getinfo", settings.rest_api_address))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(StatusCode::UNAUTHORIZED, status);

    let macaroon = fs::read(format!("{}/macaroons/admin_macaroon", settings.data_dir)).unwrap();

    let result = reqwest::Client::new()
        .get(format!("http://{}/v1/getinfo", settings.rest_api_address))
        .header("macaroon", macaroon)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    let info: GetInfo = serde_json::from_str(&result).unwrap();
    assert_eq!(mock_lightning.num_peers, info.num_peers);
}
