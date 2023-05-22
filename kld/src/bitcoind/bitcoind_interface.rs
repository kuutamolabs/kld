use anyhow::Result;
use async_trait::async_trait;
use bitcoincore_rpc_json::GetBlockchainInfoResult;

use super::bitcoind_client::MempoolInfo;

#[async_trait]
pub trait BitcoindInterface: Send + Sync {
    async fn get_blockchain_info(&self) -> Result<GetBlockchainInfoResult>;

    async fn get_mempool_info(&self) -> Result<MempoolInfo>;

    fn fee_rates_kw(&self) -> (u32, u32, u32);

    async fn block_height(&self) -> Result<u64>;
}
