use std::sync::{Arc, Weak};

use crate::logger::KldLogger;
use bitcoin::{blockdata::constants::genesis_block, BlockHash};
use lightning::routing::{
    gossip::P2PGossipSync,
    utxo::{UtxoFuture, UtxoLookup, UtxoLookupError, UtxoResult},
};
use lightning_block_sync::{BlockData, BlockSource};
use log::warn;
use settings::Settings;

use crate::ldk::{
    channel_utils::{block_from_scid, tx_index_from_scid, vout_from_scid},
    NetworkGraph,
};

use super::BitcoindClient;

pub struct BitcoindUtxoLookup {
    bitcoind: Arc<BitcoindClient>,
    network_graph: Arc<NetworkGraph>,
    gossip_sync: Weak<P2PGossipSync<Arc<NetworkGraph>, Arc<BitcoindUtxoLookup>, Arc<KldLogger>>>,
    genesis: BlockHash,
}

impl BitcoindUtxoLookup {
    pub fn new(
        settings: &Settings,
        bitcoind: Arc<BitcoindClient>,
        network_graph: Arc<NetworkGraph>,
        gossip_sync: Weak<
            P2PGossipSync<Arc<NetworkGraph>, Arc<BitcoindUtxoLookup>, Arc<KldLogger>>,
        >,
    ) -> BitcoindUtxoLookup {
        let genesis = genesis_block(settings.bitcoin_network.into())
            .header
            .block_hash();
        BitcoindUtxoLookup {
            bitcoind,
            network_graph,
            gossip_sync,
            genesis,
        }
    }
}

impl UtxoLookup for BitcoindUtxoLookup {
    fn get_utxo(&self, genesis_hash: &BlockHash, short_channel_id: u64) -> UtxoResult {
        if *genesis_hash != self.genesis {
            return UtxoResult::Sync(Err(UtxoLookupError::UnknownChain));
        }
        let async_result = UtxoFuture::new();
        let result = async_result.clone();
        let network_graph = self.network_graph.clone();
        let bitcoind = self.bitcoind.clone();
        let gossip_sync = self.gossip_sync.clone();
        tokio::spawn(async move {
            let resolve = |utxo| {
                if let Some(gossip_sync) = gossip_sync.upgrade() {
                    result.resolve(&network_graph, gossip_sync, utxo);
                } else {
                    result.resolve_without_forwarding(&network_graph, utxo);
                }
            };
            let height = block_from_scid(&short_channel_id);
            let index = tx_index_from_scid(&short_channel_id);
            let vout = vout_from_scid(&short_channel_id);
            let block_hash = match bitcoind.get_block_hash(height).await {
                Ok(hash) => hash,
                Err(e) => {
                    warn!("Could not get block hash for height {height}: {e}");
                    return resolve(Err(UtxoLookupError::UnknownTx));
                }
            };
            let block = match bitcoind.as_ref().get_block(&block_hash).await {
                Ok(BlockData::FullBlock(block)) => block,
                _ => {
                    warn!("Could not get block with hash {block_hash}");
                    return resolve(Err(UtxoLookupError::UnknownTx));
                }
            };
            if let Some(tx) = block.txdata.get(index as usize) {
                if let Some(utxo) = tx.output.get(vout as usize) {
                    return resolve(Ok(utxo.clone()));
                }
            }
            resolve(Err(UtxoLookupError::UnknownTx))
        });
        UtxoResult::Async(async_result)
    }
}
