use std::time::Duration;

use anyhow::Result;
use bitcoin::Address;
use kld::bitcoind::BitcoindClient;
use settings::Settings;

mod start;

pub const START_N_BLOCKS: u64 = 10;

pub async fn generate_blocks(
    settings: &Settings,
    n_blocks: u64,
    address: &Address,
    delay: bool,
) -> Result<()> {
    let bitcoin_client = &BitcoindClient::new(settings).await?;

    for _ in 0..n_blocks {
        bitcoin_client.generate_to_address(1, address).await?;
        bitcoin_client.wait_for_blockchain_synchronisation().await;
        // Sometimes a delay is needed to make the test more realistic which is expected by LDK.
        if delay {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    Ok(())
}
