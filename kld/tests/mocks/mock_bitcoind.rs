use std::{collections::HashMap, str::FromStr};

use anyhow::Result;
use async_trait::async_trait;
use bitcoin::BlockHash;
use bitcoincore_rpc_json::GetBlockchainInfoResult;
use kld::bitcoind::{bitcoind_interface::BitcoindInterface, MempoolInfo};
use settings::Network;
use test_utils::TEST_BLOCK_HASH;

#[derive(Default)]
pub struct MockBitcoind;

#[async_trait]
impl BitcoindInterface for MockBitcoind {
    async fn get_blockchain_info(&self) -> Result<GetBlockchainInfoResult> {
        Ok(GetBlockchainInfoResult {
            chain: Network::Regtest.to_string(),
            warnings: String::new(),
            blocks: 800000,
            headers: 800000,
            median_time: 3498239394,
            size_on_disk: 100000000,
            best_block_hash: BlockHash::from_str(TEST_BLOCK_HASH)?,
            difficulty: 340932094f64,
            verification_progress: 1f64,
            initial_block_download: false,
            pruned: false,
            chain_work: vec![],
            prune_height: None,
            prune_target_size: None,
            automatic_pruning: None,
            softforks: HashMap::new(),
        })
    }

    async fn get_mempool_info(&self) -> Result<MempoolInfo> {
        Ok(MempoolInfo {
            mempool_min_fee: 0.00003101,
        })
    }

    fn fee_rates_kw(&self) -> (u32, u32, u32) {
        (400000, 200000, 100000)
    }

    async fn block_height(&self) -> Result<u64> {
        Ok(800000)
    }
}
