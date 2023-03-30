use std::str::FromStr;

use crate::test_settings;
use anyhow::Result;
use bitcoin::Address;
use kld::bitcoind::BitcoindClient;
use settings::Settings;
use test_utils::{
    bitcoin, cockroach, kld, BitcoinManager, CockroachManager, KldManager, TEST_ADDRESS,
};

mod start;

pub const START_N_BLOCKS: u64 = 6;

pub async fn start_all(instance: &str) -> Result<(CockroachManager, BitcoinManager, KldManager)> {
    let mut settings = test_settings(instance);
    let cockroach = cockroach!(settings);
    let bitcoin = bitcoin!(settings);
    generate_blocks(&settings, START_N_BLOCKS).await?;

    let kld = kld!(&bitcoin, &cockroach, settings);

    Ok((cockroach, bitcoin, kld))
}

async fn generate_blocks(settings: &Settings, n_blocks: u64) -> Result<()> {
    let bitcoin_client = &BitcoindClient::new(settings).await?;

    bitcoin_client
        .generate_to_address(n_blocks, &Address::from_str(TEST_ADDRESS)?)
        .await?;
    bitcoin_client.wait_for_blockchain_synchronisation().await;
    Ok(())
}
