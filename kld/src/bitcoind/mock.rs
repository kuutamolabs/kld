use std::{str::FromStr, sync::Mutex};

use async_trait::async_trait;
use bitcoin::{BlockHash, Transaction, Txid};
use lightning::chain::chaininterface::{BroadcasterInterface, ConfirmationTarget, FeeEstimator};
use lightning_block_sync::{AsyncBlockSourceResult, BlockData, BlockHeaderData, BlockSource};

use crate::Service;

pub struct MockBitcoindClient {
    broadcast_transactions: Mutex<Vec<Txid>>,
    synchronised: bool,
}

impl Default for MockBitcoindClient {
    fn default() -> Self {
        Self {
            broadcast_transactions: Default::default(),
            synchronised: true,
        }
    }
}

#[async_trait]
impl Service for MockBitcoindClient {
    async fn is_connected(&self) -> bool {
        true
    }
    async fn is_synchronised(&self) -> bool {
        self.synchronised
    }
}

impl MockBitcoindClient {
    pub fn has_broadcast(&self, txid: Txid) -> bool {
        self.broadcast_transactions.lock().unwrap().contains(&txid)
    }

    pub fn set_syncronised(&mut self, syncronised: bool) {
        self.synchronised = syncronised;
    }
}

impl BroadcasterInterface for MockBitcoindClient {
    fn broadcast_transactions(&self, txs: &[&Transaction]) {
        for tx in txs {
            self.broadcast_transactions.lock().unwrap().push(tx.txid())
        }
    }
}

impl BlockSource for MockBitcoindClient {
    fn get_header<'a>(
        &'a self,
        _header_hash: &'a BlockHash,
        _height_hint: Option<u32>,
    ) -> AsyncBlockSourceResult<'a, BlockHeaderData> {
        todo!()
    }

    fn get_block<'a>(
        &'a self,
        _header_hash: &'a BlockHash,
    ) -> AsyncBlockSourceResult<'a, BlockData> {
        todo!()
    }

    fn get_best_block<'a>(&'_ self) -> AsyncBlockSourceResult<(BlockHash, Option<u32>)> {
        Box::pin(async {
            Ok((
                BlockHash::from_str(
                    "000000000000000000015d9e9473a56a7dde8ea974f0efd2ff9bd068f052134a",
                )
                .unwrap(),
                Some(782000),
            ))
        })
    }
}

impl FeeEstimator for MockBitcoindClient {
    fn get_est_sat_per_1000_weight(&self, confirmation_target: ConfirmationTarget) -> u32 {
        match confirmation_target {
            ConfirmationTarget::Background => 500,
            ConfirmationTarget::Normal => 2000,
            ConfirmationTarget::HighPriority => 10000,
            ConfirmationTarget::MempoolMinimum => 500,
        }
    }
}
