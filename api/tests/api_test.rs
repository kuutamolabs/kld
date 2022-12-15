use std::{fs, sync::Arc};

use bitcoin::{secp256k1::PublicKey, Network};
use futures::FutureExt;
use reqwest::StatusCode;
use test_utils::random_public_key;
use test_utils::test_settings;
use tokio::signal::unix::SignalKind;

use api::{start_rest_api, GetInfo, LightningInterface, MacaroonAuth};

#[tokio::test(flavor = "multi_thread")]
async fn test_api() {
    let mut settings = test_settings();
    settings.data_dir = env!("CARGO_TARGET_TMPDIR").to_string();
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

async fn quit_signal() {
    let _ = tokio::signal::unix::signal(SignalKind::quit())
        .unwrap()
        .recv()
        .await;
}

pub struct MockLightning {
    num_peers: usize,
}

impl Default for MockLightning {
    fn default() -> Self {
        Self { num_peers: 5 }
    }
}

impl LightningInterface for MockLightning {
    fn alias(&self) -> String {
        "test".to_string()
    }
    fn identity_pubkey(&self) -> PublicKey {
        random_public_key()
    }

    fn graph_num_nodes(&self) -> usize {
        0
    }

    fn graph_num_channels(&self) -> usize {
        0
    }

    fn block_height(&self) -> usize {
        50000
    }

    fn network(&self) -> bitcoin::Network {
        Network::Bitcoin
    }
    fn num_active_channels(&self) -> usize {
        0
    }

    fn num_inactive_channels(&self) -> usize {
        0
    }

    fn num_pending_channels(&self) -> usize {
        0
    }
    fn num_peers(&self) -> usize {
        self.num_peers
    }

    fn wallet_balance(&self) -> u64 {
        0
    }

    fn version(&self) -> String {
        "v0.1".to_string()
    }
}
