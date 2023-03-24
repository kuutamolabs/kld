use lightning::chain::chaininterface::FeeEstimator;
use lightning_block_sync::BlockSource;

#[derive(Default)]
pub struct MockBitcoindClient;

impl BlockSource for MockBitcoindClient {
    fn get_header<'a>(
        &'a self,
        _header_hash: &'a bitcoin::BlockHash,
        _height_hint: Option<u32>,
    ) -> lightning_block_sync::AsyncBlockSourceResult<'a, lightning_block_sync::BlockHeaderData>
    {
        todo!()
    }

    fn get_block<'a>(
        &'a self,
        _header_hash: &'a bitcoin::BlockHash,
    ) -> lightning_block_sync::AsyncBlockSourceResult<'a, lightning_block_sync::BlockData> {
        todo!()
    }

    fn get_best_block<'a>(
        &'_ self,
    ) -> lightning_block_sync::AsyncBlockSourceResult<(bitcoin::BlockHash, Option<u32>)> {
        todo!()
    }
}

impl FeeEstimator for MockBitcoindClient {
    fn get_est_sat_per_1000_weight(
        &self,
        confirmation_target: lightning::chain::chaininterface::ConfirmationTarget,
    ) -> u32 {
        match confirmation_target {
            lightning::chain::chaininterface::ConfirmationTarget::Background => 500,
            lightning::chain::chaininterface::ConfirmationTarget::Normal => 2000,
            lightning::chain::chaininterface::ConfirmationTarget::HighPriority => 10000,
        }
    }
}
