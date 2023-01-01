use std::{fs, sync::Arc};

use futures::FutureExt;
use lightning_knd::api::start_rest_api;
use lightning_knd::api::MacaroonAuth;
use reqwest::StatusCode;
use test_utils::{https_client, TestSettingsBuilder};

use api::GetInfo;

use crate::quit_signal;
use crate::MockLightning;

#[tokio::test(flavor = "multi_thread")]
async fn test_api() {
    let settings = TestSettingsBuilder::new()
        .with_data_dir(&format!("{}/test_api", env!("CARGO_TARGET_TMPDIR")))
        .build();

    let mock_lightning = Arc::new(MockLightning::default());
    let macaroon_auth = Arc::new(MacaroonAuth::init(&[0u8; 32], &settings.data_dir).unwrap());

    tokio::spawn(start_rest_api(
        settings.rest_api_address.clone(),
        settings.certs_dir.clone(),
        mock_lightning.clone(),
        macaroon_auth,
        quit_signal().shared(),
    ));

    let client = https_client();

    let result = client
        .get(format!("https://{}/", settings.rest_api_address))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!("OK", result);

    let status = client
        .get(format!("https://{}/v1/getinfo", settings.rest_api_address))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(StatusCode::UNAUTHORIZED, status);

    let macaroon = fs::read(format!("{}/macaroons/admin_macaroon", settings.data_dir)).unwrap();

    let result = client
        .get(format!("https://{}/v1/getinfo", settings.rest_api_address))
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
