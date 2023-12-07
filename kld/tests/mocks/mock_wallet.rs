use std::{str::FromStr, vec};

use anyhow::Result;
use async_trait::async_trait;
use bdk::{wallet::AddressInfo, Balance, BlockTime, KeychainKind, LocalUtxo, TransactionDetails};
use bitcoin::{consensus::deserialize, hashes::hex::FromHex, Address, OutPoint, Transaction};
use kld::wallet::WalletInterface;

use test_utils::{TEST_ADDRESS, TEST_TX};

pub struct MockWallet {
    balance: Balance,
    transaction: Transaction,
}

#[async_trait]
impl WalletInterface for MockWallet {
    fn balance(&self) -> Result<Balance> {
        Ok(self.balance.clone())
    }

    async fn transfer(
        &self,
        _address: Address,
        amount: u64,
        _fee_rate: Option<kld::api::payloads::FeeRate>,
        _min_conf: Option<u8>,
        _utxos: Vec<OutPoint>,
    ) -> Result<(Transaction, TransactionDetails)> {
        let details = TransactionDetails {
            transaction: Some(self.transaction.clone()),
            txid: self.transaction.txid(),
            received: 0,
            sent: amount,
            fee: None,
            confirmation_time: None,
        };
        Ok((self.transaction.clone(), details))
    }

    fn new_external_address(&self) -> Result<AddressInfo> {
        Ok(AddressInfo {
            address: Address::from_str(TEST_ADDRESS)?,
            index: 1,
            keychain: KeychainKind::External,
        })
    }

    fn new_internal_address(&self) -> Result<AddressInfo> {
        Ok(AddressInfo {
            address: Address::from_str(TEST_ADDRESS)?,
            index: 1,
            keychain: KeychainKind::Internal,
        })
    }

    fn list_utxos(&self) -> Result<Vec<(LocalUtxo, TransactionDetails)>> {
        let details = TransactionDetails {
            transaction: Some(self.transaction.clone()),
            txid: self.transaction.txid(),
            received: 10000,
            sent: 1200,
            fee: Some(20),
            confirmation_time: BlockTime::new(Some(600000), Some(23293219)),
        };
        let utxo = LocalUtxo {
            outpoint: OutPoint::new(self.transaction.txid(), 0),
            txout: self.transaction.output.get(0).unwrap().clone(),
            keychain: KeychainKind::External,
            is_spent: false,
        };
        Ok(vec![(utxo, details)])
    }
}

impl Default for MockWallet {
    fn default() -> Self {
        let transaction =
            deserialize::<bitcoin::Transaction>(&Vec::<u8>::from_hex(TEST_TX).unwrap()).unwrap();
        Self {
            balance: Balance {
                immature: 1,
                trusted_pending: 2,
                untrusted_pending: 3,
                confirmed: 4,
            },
            transaction,
        }
    }
}
