mod bitcoind_client;
mod utxo_lookup;

use anyhow::Result;
use async_trait::async_trait;
pub use bitcoind_client::BitcoindClient;
pub use utxo_lookup::BitcoindUtxoLookup;

#[cfg(test)]
pub mod mock;
#[cfg(test)]
pub use mock::MockBitcoindClient;

#[async_trait]
pub trait Synchronised {
    async fn is_available(&self) -> bool;
    async fn is_synchronised(&self) -> Result<bool>;
}
