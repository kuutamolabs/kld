use anyhow::Result;
use api::FeeRate;
use async_trait::async_trait;
use bdk::{wallet::AddressInfo, Balance, LocalUtxo, TransactionDetails};
use bitcoin::{Address, OutPoint, Transaction};

#[async_trait]
pub trait WalletInterface {
    fn balance(&self) -> Result<Balance>;

    /// Set amount to u64::MAX to drain the wallet.
    async fn transfer(
        &self,
        address: Address,
        amount: u64,
        fee_rate: Option<FeeRate>,
        min_conf: Option<u8>,
        utxos: Vec<OutPoint>,
    ) -> Result<(Transaction, TransactionDetails)>;

    fn new_external_address(&self) -> Result<AddressInfo>;

    fn new_internal_address(&self) -> Result<AddressInfo>;

    fn list_utxos(&self) -> Result<Vec<(LocalUtxo, TransactionDetails)>>;
}
