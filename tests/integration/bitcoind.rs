use std::str::FromStr;

use anyhow::{anyhow, bail, Result};
use bitcoin::Address;
use lightning::chain::chaininterface::{ConfirmationTarget, FeeEstimator};
use lightning_block_sync::{BlockData, BlockSource};
use lightning_knd::bitcoind::BitcoindClient;
use test_utils::{bitcoin, TestSettingsBuilder};

use crate::mocks::TEST_ADDRESS;

#[tokio::test(flavor = "multi_thread")]
pub async fn test_bitcoind_client() -> Result<()> {
    let mut bitcoin = bitcoin!();
    bitcoin.start().await?;

    let settings = TestSettingsBuilder::new().with_bitcoind(&bitcoin)?.build();

    let client = &BitcoindClient::new(&settings).await?;

    let n_blocks = 3;
    client
        .generate_to_address(n_blocks, &Address::from_str(TEST_ADDRESS).unwrap())
        .await?;

    client.wait_for_blockchain_synchronisation().await?;

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
        232,
        client.get_est_sat_per_1000_weight(ConfirmationTarget::Background)
    );
    assert_eq!(
        500,
        client.get_est_sat_per_1000_weight(ConfirmationTarget::Normal)
    );
    assert_eq!(
        1250,
        client.get_est_sat_per_1000_weight(ConfirmationTarget::HighPriority)
    );

    match block {
        BlockData::FullBlock(block) => assert_eq!(block.block_hash(), best_block.0),
        BlockData::HeaderOnly(_header) => bail!("Should be a full block"),
    };
    Ok(())
}
