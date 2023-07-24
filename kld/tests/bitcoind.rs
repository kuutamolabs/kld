use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Result};
use bitcoin::Address;
use kld::bitcoind::bitcoind_interface::BitcoindInterface;
use lightning::chain::chaininterface::{ConfirmationTarget, FeeEstimator};
use lightning_block_sync::{BlockData, BlockSource};
use test_utils::{bitcoin, test_settings, TEST_ADDRESS};

#[tokio::test(flavor = "multi_thread")]
pub async fn test_bitcoind_client() -> Result<()> {
    let mut settings = test_settings!("client");
    let bitcoind = bitcoin!(settings);
    let client = bitcoind.client.get().context("expected client")?;
    let n_blocks = 3;
    client
        .generate_to_address(n_blocks, &Address::from_str(TEST_ADDRESS).unwrap())
        .await?;

    client.wait_for_blockchain_synchronisation().await;

    let info = client.get_blockchain_info().await?;
    assert_eq!(n_blocks, info.blocks);

    let best_block = client
        .get_best_block()
        .await
        .map_err(|e| anyhow!(e.into_inner()))?;
    assert_eq!(best_block.0, info.best_block_hash);
    assert_eq!(best_block.1, Some(n_blocks as u32));

    let header = client
        .get_header(&best_block.0, None)
        .await
        .map_err(|e| anyhow!(e.into_inner()))?;
    assert_eq!(header.height, n_blocks as u32);
    assert_eq!(header.chainwork.low_u64(), 8);

    let block = &client
        .get_block(&best_block.0)
        .await
        .map_err(|e| anyhow!(e.into_inner()))?;

    assert_eq!(
        253,
        client.get_est_sat_per_1000_weight(ConfirmationTarget::Background)
    );
    assert_eq!(
        2000,
        client.get_est_sat_per_1000_weight(ConfirmationTarget::Normal)
    );
    assert_eq!(
        5000,
        client.get_est_sat_per_1000_weight(ConfirmationTarget::HighPriority)
    );

    client.poll_for_fee_estimates();

    match block {
        BlockData::FullBlock(block) => assert_eq!(block.block_hash(), best_block.0),
        BlockData::HeaderOnly(_header) => bail!("Should be a full block"),
    };
    Ok(())
}
