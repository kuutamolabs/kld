use std::str::FromStr;

use anyhow::Result;
use async_trait::async_trait;
use bdk::{wallet::AddressInfo, Balance, KeychainKind, TransactionDetails};
use bitcoin::{consensus::deserialize, hashes::hex::FromHex, Address, OutPoint, Transaction};
use kld::wallet::WalletInterface;

use test_utils::{TEST_ADDRESS, TEST_TX};

pub struct MockWallet {
    balance: Balance,
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
        _fee_rate: Option<api::FeeRate>,
        _min_conf: Option<u8>,
        _utxos: Vec<OutPoint>,
    ) -> Result<(Transaction, TransactionDetails)> {
        let transaction =
            deserialize::<bitcoin::Transaction>(&Vec::<u8>::from_hex(TEST_TX).unwrap()).unwrap();
        let details = TransactionDetails {
            transaction: Some(transaction.clone()),
            txid: transaction.txid(),
            received: 0,
            sent: amount,
            fee: None,
            confirmation_time: None,
        };
        Ok((transaction, details))
    }

    fn new_address(&self) -> Result<AddressInfo> {
        Ok(AddressInfo {
            address: Address::from_str(TEST_ADDRESS).unwrap(),
            index: 1,
            keychain: KeychainKind::External,
        })
    }
}

impl Default for MockWallet {
    fn default() -> Self {
        Self {
            balance: Balance {
                immature: 1,
                trusted_pending: 2,
                untrusted_pending: 3,
                confirmed: 4,
            },
        }
    }
}
