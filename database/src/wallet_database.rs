use std::sync::Arc;

use crate::{connection, to_i64, Client};
use anyhow::Result;
use bdk::{
    database::{BatchDatabase, BatchOperations, Database, SyncTime},
    BlockTime, Error, KeychainKind, LocalUtxo, TransactionDetails,
};
use bitcoin::consensus::encode::{deserialize, serialize};
use bitcoin::{OutPoint, Script, Transaction, TxOut, Txid};
use log::info;
use settings::Settings;
use tokio::{runtime::Handle, sync::RwLock};

macro_rules! execute_blocking {
    ($statement: literal, $params: expr, $self: expr) => {
        tokio::task::block_in_place(move || {
            Handle::current().block_on(async move {
                $self
                    .client()
                    .await
                    .map_err(|e| Error::Generic(e.to_string()))?
                    .read()
                    .await
                    .execute($statement, $params)
                    .await
                    .map_err(|e| Error::Generic(e.to_string()))
            })
        })
    };
}

macro_rules! query_blocking {
    ($statement: literal, $params: expr, $self: expr) => {
        tokio::task::block_in_place(move || {
            Handle::current().block_on(async move {
                $self
                    .client()
                    .await
                    .map_err(|e| Error::Generic(e.to_string()))?
                    .read()
                    .await
                    .query($statement, $params)
                    .await
                    .map_err(|e| Error::Generic(e.to_string()))
            })
        })
    };
}

#[derive(Clone)]
pub struct WalletDatabase {
    settings: Settings,
    client: Arc<RwLock<Client>>,
}

impl WalletDatabase {
    pub async fn new(settings: &Settings) -> Result<WalletDatabase> {
        info!(
            "Connecting wallet to Cockroach database at {}:{}",
            settings.database_host, settings.database_port
        );
        let client = connection(settings).await?;
        Ok(WalletDatabase {
            settings: settings.clone(),
            client: Arc::new(RwLock::new(client)),
        })
    }

    /// Try to reconnect to the database if the connection has been dropped.
    /// If this is not possible one of the callers of this function should shut the node down.
    async fn client(&self) -> Result<Arc<RwLock<Client>>> {
        if self.client.read().await.is_closed() {
            let mut guard = self.client.write().await;
            if guard.is_closed() {
                let client = connection(&self.settings).await?;
                *guard = client;
            }
        }
        Ok(self.client.clone())
    }

    fn insert_script_pubkey(
        &self,
        keychain: String,
        child: u32,
        script: &[u8],
    ) -> Result<i64, Error> {
        execute_blocking!(
            "INSERT INTO wallet_script_pubkeys (keychain, child, script) VALUES ($1, $2, $3)",
            &[&keychain, &to_i64!(child), &script],
            self
        )
        .map(|_| 0)
    }

    fn insert_utxo(
        &self,
        value: u64,
        keychain: String,
        vout: u32,
        txid: &[u8],
        script: &[u8],
        is_spent: bool,
    ) -> Result<i64, Error> {
        execute_blocking!(
			"UPSERT INTO wallet_utxos (value, keychain, vout, txid, script, is_spent) VALUES ($1, $2, $3, $4, $5, $6)",
			&[&to_i64!(value), &keychain, &to_i64!(vout), &txid, &script, &is_spent],
			self
		)
		.map(|_| 0)
    }

    fn insert_transaction(&self, txid: &[u8], raw_tx: &[u8]) -> Result<i64, Error> {
        execute_blocking!(
            "INSERT INTO wallet_transactions (txid, raw_tx) VALUES ($1, $2)",
            &[&txid, &raw_tx],
            self
        )
        .map(|_| 0)
    }

    fn update_transaction(&self, txid: &[u8], raw_tx: &[u8]) -> Result<(), Error> {
        execute_blocking!(
            "UPDATE wallet_transactions SET raw_tx=$1 WHERE txid=$2",
            &[&txid, &raw_tx],
            self
        )
        .map(|_| ())
    }

    fn insert_transaction_details(&self, transaction: &TransactionDetails) -> Result<i64, Error> {
        let (timestamp, height) = match &transaction.confirmation_time {
            Some(confirmation_time) => (
                Some(confirmation_time.timestamp),
                Some(confirmation_time.height),
            ),
            None => (None, None),
        };

        let txid: &[u8] = &transaction.txid;

        execute_blocking!(
			"INSERT INTO wallet_transaction_details (txid, timestamp, received, sent, fee, height) VALUES ($1, $2, $3, $4, $5, $6)",
			&[
				&txid,
				&timestamp.map(|x| to_i64!(x)),
				&to_i64!(transaction.received),
				&to_i64!(transaction.sent),
				&transaction.fee.map(|x| to_i64!(x)),
				&height.map(|x| to_i64!(x))
			],
			self
		)
		.map(|_| 0)
    }

    fn update_transaction_details(&self, transaction: &TransactionDetails) -> Result<(), Error> {
        let (timestamp, height) = match &transaction.confirmation_time {
            Some(confirmation_time) => (
                Some(confirmation_time.timestamp),
                Some(confirmation_time.height),
            ),
            None => (None, None),
        };

        let txid: &[u8] = &transaction.txid;

        execute_blocking!(
			"UPDATE wallet_transaction_details SET timestamp=$1, received=$2, sent=$3, fee=$4, height=$5 WHERE txid=$6",
			&[
				&timestamp.map(|x| to_i64!(x)),
				&to_i64!(transaction.received),
				&to_i64!(transaction.sent),
				&transaction.fee.map(|x| to_i64!(x)),
				&height.map(|x| to_i64!(x)),
				&txid,
			],
			self
		)
		.map(|_| ())
    }

    fn insert_last_derivation_index(&self, keychain: String, value: u32) -> Result<i64, Error> {
        execute_blocking!(
            "INSERT INTO wallet_last_derivation_indices (keychain, value) VALUES ($1, $2)",
            &[&keychain, &to_i64!(value)],
            self
        )
        .map(|_| 0)
    }

    fn insert_checksum(&self, keychain: String, checksum: &[u8]) -> Result<i64, Error> {
        execute_blocking!(
            "INSERT INTO wallet_checksums (keychain, checksum) VALUES ($1, $2)",
            &[&keychain, &checksum],
            self
        )
        .map(|_| 0)
    }

    fn update_last_derivation_index(&self, keychain: String, value: u32) -> Result<(), Error> {
        execute_blocking!(
            "UPSERT INTO wallet_last_derivation_indices (keychain, value) VALUES ($1, $2)",
            &[&keychain, &to_i64!(value)],
            self
        )
        .map(|_| ())
    }

    fn update_sync_time(&self, data: SyncTime) -> Result<i64, Error> {
        execute_blocking!(
            "UPSERT INTO wallet_sync_time (id, height, timestamp) VALUES (0, $1, $2)",
            &[
                &to_i64!(data.block_time.height),
                &to_i64!(data.block_time.timestamp)
            ],
            self
        )
        .map(|_| 0)
    }

    fn select_script_pubkeys(&self) -> Result<Vec<Script>, Error> {
        let rows = query_blocking!("SELECT script FROM wallet_script_pubkeys", &[], self)?;
        let mut scripts: Vec<Script> = vec![];
        for row in rows {
            let raw_script: Vec<u8> = row.get(0);
            scripts.push(raw_script.into());
        }
        Ok(scripts)
    }

    fn select_script_pubkeys_by_keychain(&self, keychain: String) -> Result<Vec<Script>, Error> {
        let rows = query_blocking!(
            "SELECT script FROM wallet_script_pubkeys WHERE keychain=$1",
            &[&keychain],
            self
        )?;
        let mut scripts: Vec<Script> = vec![];
        for row in rows {
            let raw_script: Vec<u8> = row.get(0);
            scripts.push(raw_script.into());
        }
        Ok(scripts)
    }

    fn select_script_pubkey_by_path(
        &self,
        keychain: String,
        child: u32,
    ) -> Result<Option<Script>, Error> {
        let rows = query_blocking!(
            "SELECT script FROM wallet_script_pubkeys WHERE keychain=$1 AND child=$2",
            &[&keychain, &to_i64!(child)],
            self
        )?;

        match rows.get(0) {
            Some(row) => {
                let script: Vec<u8> = row.get(0);
                let script: Script = script.into();
                Ok(Some(script))
            }
            None => Ok(None),
        }
    }

    fn select_script_pubkey_by_script(
        &self,
        script: &[u8],
    ) -> Result<Option<(KeychainKind, u32)>, Error> {
        let rows = query_blocking!(
            "SELECT keychain, child FROM wallet_script_pubkeys WHERE script=$1",
            &[&script],
            self
        )?;
        match rows.get(0) {
            Some(row) => {
                let keychain: String = row.get(0);
                let keychain: KeychainKind = serde_json::from_str(&keychain)?;
                let child: u32 = row.get::<usize, i64>(1).try_into().unwrap();
                Ok(Some((keychain, child)))
            }
            None => Ok(None),
        }
    }

    fn select_utxos(&self) -> Result<Vec<LocalUtxo>, Error> {
        let rows = query_blocking!(
            "SELECT value, keychain, vout, txid, script, is_spent FROM wallet_utxos",
            &[],
            self
        )?;
        let mut utxos: Vec<LocalUtxo> = vec![];
        for row in rows {
            let value: u64 = row.get::<usize, i64>(0).try_into().unwrap();
            let keychain: String = row.get(1);
            let vout: u32 = row.get::<usize, i64>(2).try_into().unwrap();
            let txid: Vec<u8> = row.get(3);
            let script: Vec<u8> = row.get(4);
            let is_spent: bool = row.get(5);

            let keychain: KeychainKind = serde_json::from_str(&keychain)?;

            utxos.push(LocalUtxo {
                outpoint: OutPoint::new(deserialize(&txid)?, vout),
                txout: TxOut {
                    value,
                    script_pubkey: script.into(),
                },
                keychain,
                is_spent,
            })
        }
        Ok(utxos)
    }

    fn select_utxo_by_outpoint(&self, txid: &[u8], vout: u32) -> Result<Option<LocalUtxo>, Error> {
        let rows = query_blocking!(
            "SELECT value, keychain, script, is_spent FROM wallet_utxos WHERE txid=$1 AND vout=$2",
            &[&txid, &to_i64!(vout)],
            self
        )?;
        match rows.get(0) {
            Some(row) => {
                let value: u64 = row.get::<usize, i64>(0).try_into().unwrap();
                let keychain: String = row.get(1);
                let keychain: KeychainKind = serde_json::from_str(&keychain)?;
                let script: Vec<u8> = row.get(2);
                let script_pubkey: Script = script.into();
                let is_spent: bool = row.get(3);

                Ok(Some(LocalUtxo {
                    outpoint: OutPoint::new(deserialize(txid)?, vout),
                    txout: TxOut {
                        value,
                        script_pubkey,
                    },
                    keychain,
                    is_spent,
                }))
            }
            None => Ok(None),
        }
    }

    fn select_transactions(&self) -> Result<Vec<Transaction>, Error> {
        let rows = query_blocking!("SELECT raw_tx FROM wallet_transactions", &[], self)?;
        let mut txs: Vec<Transaction> = vec![];
        for row in rows {
            let raw_tx: Vec<u8> = row.get(0);
            let tx: Transaction = deserialize(&raw_tx)?;
            txs.push(tx);
        }
        Ok(txs)
    }

    fn select_transaction_by_txid(&self, txid: &[u8]) -> Result<Option<Transaction>, Error> {
        let rows = query_blocking!(
            "SELECT raw_tx FROM wallet_transactions WHERE txid=$1",
            &[&txid],
            self
        )?;
        match rows.get(0) {
            Some(row) => {
                let raw_tx: Vec<u8> = row.get(0);
                let tx: Transaction = deserialize(&raw_tx)?;
                Ok(Some(tx))
            }
            None => Ok(None),
        }
    }

    fn select_transaction_details_with_raw(&self) -> Result<Vec<TransactionDetails>, Error> {
        let rows = query_blocking!("SELECT wtd.txid, wtd.timestamp, wtd.received, wtd.sent, wtd.fee, wtd.height, wt.raw_tx FROM wallet_transaction_details wtd, wallet_transactions wt WHERE wtd.txid = wt.txid", &[], self)?;
        let mut transaction_details: Vec<TransactionDetails> = vec![];
        for row in rows {
            let txid: Vec<u8> = row.get(0);
            let txid: Txid = deserialize(&txid)?;
            let timestamp: Option<u64> = row
                .get::<usize, Option<i64>>(1)
                .map(|x| x.try_into().unwrap());
            let received: u64 = row.get::<usize, i64>(2).try_into().unwrap();
            let sent: u64 = row.get::<usize, i64>(3).try_into().unwrap();
            let fee: Option<u64> = row
                .get::<usize, Option<i64>>(4)
                .map(|x| x.try_into().unwrap());
            let height: Option<u32> = row
                .get::<usize, Option<i64>>(5)
                .map(|x| x.try_into().unwrap());
            let raw_tx: Option<Vec<u8>> = row.get(6);
            let tx: Option<Transaction> = match raw_tx {
                Some(raw_tx) => {
                    let tx: Transaction = deserialize(&raw_tx)?;
                    Some(tx)
                }
                None => None,
            };

            let confirmation_time = match (height, timestamp) {
                (Some(height), Some(timestamp)) => Some(BlockTime { height, timestamp }),
                _ => None,
            };

            transaction_details.push(TransactionDetails {
                transaction: tx,
                txid,
                received,
                sent,
                fee,
                confirmation_time,
            });
        }
        Ok(transaction_details)
    }

    fn select_transaction_details(&self) -> Result<Vec<TransactionDetails>, Error> {
        let rows = query_blocking!(
            "SELECT txid, timestamp, received, sent, fee, height FROM wallet_transaction_details",
            &[],
            self
        )?;
        let mut transaction_details: Vec<TransactionDetails> = vec![];
        for row in rows {
            let txid: Vec<u8> = row.get(0);
            let txid: Txid = deserialize(&txid)?;
            let timestamp: Option<u64> = row
                .get::<usize, Option<i64>>(1)
                .map(|x| x.try_into().unwrap());
            let received: u64 = row.get::<usize, i64>(2).try_into().unwrap();
            let sent: u64 = row.get::<usize, i64>(3).try_into().unwrap();
            let fee: Option<u64> = row
                .get::<usize, Option<i64>>(4)
                .map(|x| x.try_into().unwrap());
            let height: Option<u32> = row
                .get::<usize, Option<i64>>(5)
                .map(|x| x.try_into().unwrap());

            let confirmation_time = match (height, timestamp) {
                (Some(height), Some(timestamp)) => Some(BlockTime { height, timestamp }),
                _ => None,
            };

            transaction_details.push(TransactionDetails {
                transaction: None,
                txid,
                received,
                sent,
                fee,
                confirmation_time,
            });
        }
        Ok(transaction_details)
    }

    fn select_transaction_details_by_txid(
        &self,
        txid: &[u8],
    ) -> Result<Option<TransactionDetails>, Error> {
        let rows = query_blocking!("SELECT wtd.timestamp, wtd.received, wtd.sent, wtd.fee, wtd.height, wt.raw_tx FROM wallet_transaction_details wtd, wallet_transactions wt WHERE wtd.txid=wt.txid AND wtd.txid=$1", &[&txid], self)?;

        match rows.get(0) {
            Some(row) => {
                let timestamp: Option<u64> = row
                    .get::<usize, Option<i64>>(0)
                    .map(|x| x.try_into().unwrap());
                let received: u64 = row.get::<usize, i64>(1).try_into().unwrap();
                let sent: u64 = row.get::<usize, i64>(2).try_into().unwrap();
                let fee: Option<u64> = row
                    .get::<usize, Option<i64>>(3)
                    .map(|x| x.try_into().unwrap());
                let height: Option<u32> = row
                    .get::<usize, Option<i64>>(4)
                    .map(|x| x.try_into().unwrap());

                let raw_tx: Option<Vec<u8>> = row.get(5);
                let tx: Option<Transaction> = match raw_tx {
                    Some(raw_tx) => {
                        let tx: Transaction = deserialize(&raw_tx)?;
                        Some(tx)
                    }
                    None => None,
                };

                let confirmation_time = match (height, timestamp) {
                    (Some(height), Some(timestamp)) => Some(BlockTime { height, timestamp }),
                    _ => None,
                };

                Ok(Some(TransactionDetails {
                    transaction: tx,
                    txid: deserialize(txid)?,
                    received,
                    sent,
                    fee,
                    confirmation_time,
                }))
            }
            None => Ok(None),
        }
    }

    fn select_last_derivation_index_by_keychain(
        &self,
        keychain: String,
    ) -> Result<Option<u32>, Error> {
        let rows = query_blocking!(
            "SELECT value FROM wallet_last_derivation_indices WHERE keychain=$1",
            &[&keychain],
            self
        )?;
        match rows.get(0) {
            Some(row) => {
                let value: u32 = row.get::<usize, i64>(0).try_into().unwrap();
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    fn select_sync_time(&self) -> Result<Option<SyncTime>, Error> {
        let rows = query_blocking!(
            "SELECT height, timestamp FROM wallet_sync_time WHERE id = 0",
            &[],
            self
        )?;

        if let Some(row) = rows.get(0) {
            Ok(Some(SyncTime {
                block_time: BlockTime {
                    height: row.get::<usize, i64>(0).try_into().unwrap(),
                    timestamp: row.get::<usize, i64>(1).try_into().unwrap(),
                },
            }))
        } else {
            Ok(None)
        }
    }

    fn select_checksum_by_keychain(&self, keychain: String) -> Result<Option<Vec<u8>>, Error> {
        let rows = query_blocking!(
            "SELECT checksum FROM wallet_checksums WHERE keychain=$1",
            &[&keychain],
            self
        )?;

        match rows.get(0) {
            Some(row) => {
                let checksum: Vec<u8> = row.get(0);
                Ok(Some(checksum))
            }
            None => Ok(None),
        }
    }

    fn delete_script_pubkey_by_path(&self, keychain: String, child: u32) -> Result<(), Error> {
        execute_blocking!(
            "DELETE FROM wallet_script_pubkeys WHERE keychain=$1 AND child=$2",
            &[&keychain, &to_i64!(child)],
            self
        )
        .map(|_| ())
    }

    fn delete_script_pubkey_by_script(&self, script: &[u8]) -> Result<(), Error> {
        execute_blocking!(
            "DELETE FROM wallet_script_pubkeys WHERE script=$1",
            &[&script],
            self
        )
        .map(|_| ())
    }

    fn delete_utxo_by_outpoint(&self, txid: &[u8], vout: u32) -> Result<(), Error> {
        execute_blocking!(
            "DELETE FROM wallet_utxos WHERE txid=$1 AND vout=$2",
            &[&txid, &to_i64!(vout)],
            self
        )
        .map(|_| ())
    }

    fn delete_transaction_by_txid(&self, txid: &[u8]) -> Result<(), Error> {
        execute_blocking!(
            "DELETE FROM wallet_transactions WHERE txid=$1",
            &[&txid],
            self
        )
        .map(|_| ())
    }

    fn delete_transaction_details_by_txid(&self, txid: &[u8]) -> Result<(), Error> {
        execute_blocking!(
            "DELETE FROM wallet_transaction_details WHERE txid=$1",
            &[&txid],
            self
        )
        .map(|_| ())
    }

    fn delete_last_derivation_index_by_keychain(&self, keychain: String) -> Result<(), Error> {
        execute_blocking!(
            "DELETE FROM wallet_last_derivation_indices WHERE keychain=$1",
            &[&keychain],
            self
        )
        .map(|_| ())
    }

    fn delete_sync_time(&self) -> Result<(), Error> {
        execute_blocking!("DELETE FROM wallet_sync_time WHERE id = 0", &[], self).map(|_| ())
    }
}

impl BatchOperations for WalletDatabase {
    fn set_script_pubkey(
        &mut self,
        script: &Script,
        keychain: KeychainKind,
        child: u32,
    ) -> Result<(), Error> {
        let keychain = serde_json::to_string(&keychain)?;
        self.insert_script_pubkey(keychain, child, script.as_bytes())?;
        Ok(())
    }

    fn set_utxo(&mut self, utxo: &LocalUtxo) -> Result<(), Error> {
        self.insert_utxo(
            utxo.txout.value,
            serde_json::to_string(&utxo.keychain)?,
            utxo.outpoint.vout,
            &utxo.outpoint.txid,
            utxo.txout.script_pubkey.as_bytes(),
            utxo.is_spent,
        )?;
        Ok(())
    }

    fn set_raw_tx(&mut self, transaction: &Transaction) -> Result<(), Error> {
        match self.select_transaction_by_txid(&transaction.txid())? {
            Some(_) => {
                self.update_transaction(&transaction.txid(), &serialize(transaction))?;
            }
            None => {
                self.insert_transaction(&transaction.txid(), &serialize(transaction))?;
            }
        }
        Ok(())
    }

    fn set_tx(&mut self, transaction: &TransactionDetails) -> Result<(), Error> {
        match self.select_transaction_details_by_txid(&transaction.txid)? {
            Some(_) => {
                self.update_transaction_details(transaction)?;
            }
            None => {
                self.insert_transaction_details(transaction)?;
            }
        }

        if let Some(tx) = &transaction.transaction {
            self.set_raw_tx(tx)?;
        }

        Ok(())
    }

    fn set_last_index(&mut self, keychain: KeychainKind, value: u32) -> Result<(), Error> {
        self.update_last_derivation_index(serde_json::to_string(&keychain)?, value)?;
        Ok(())
    }

    fn set_sync_time(&mut self, ct: SyncTime) -> Result<(), Error> {
        self.update_sync_time(ct)?;
        Ok(())
    }

    fn del_script_pubkey_from_path(
        &mut self,
        keychain: KeychainKind,
        child: u32,
    ) -> Result<Option<Script>, Error> {
        let keychain = serde_json::to_string(&keychain)?;
        let script = self.select_script_pubkey_by_path(keychain.clone(), child)?;
        match script {
            Some(script) => {
                self.delete_script_pubkey_by_path(keychain, child)?;
                Ok(Some(script))
            }
            None => Ok(None),
        }
    }

    fn del_path_from_script_pubkey(
        &mut self,
        script: &Script,
    ) -> Result<Option<(KeychainKind, u32)>, Error> {
        match self.select_script_pubkey_by_script(script.as_bytes())? {
            Some((keychain, child)) => {
                self.delete_script_pubkey_by_script(script.as_bytes())?;
                Ok(Some((keychain, child)))
            }
            None => Ok(None),
        }
    }

    fn del_utxo(&mut self, outpoint: &OutPoint) -> Result<Option<LocalUtxo>, Error> {
        match self.select_utxo_by_outpoint(&outpoint.txid, outpoint.vout)? {
            Some(local_utxo) => {
                self.delete_utxo_by_outpoint(&outpoint.txid, outpoint.vout)?;
                Ok(Some(local_utxo))
            }
            None => Ok(None),
        }
    }

    fn del_raw_tx(&mut self, txid: &Txid) -> Result<Option<Transaction>, Error> {
        match self.select_transaction_by_txid(txid)? {
            Some(tx) => {
                self.delete_transaction_by_txid(txid)?;
                Ok(Some(tx))
            }
            None => Ok(None),
        }
    }

    fn del_tx(
        &mut self,
        txid: &Txid,
        include_raw: bool,
    ) -> Result<Option<TransactionDetails>, Error> {
        match self.select_transaction_details_by_txid(txid)? {
            Some(transaction_details) => {
                self.delete_transaction_details_by_txid(txid)?;

                if include_raw {
                    self.delete_transaction_by_txid(txid)?;
                }
                Ok(Some(transaction_details))
            }
            None => Ok(None),
        }
    }

    fn del_last_index(&mut self, keychain: KeychainKind) -> Result<Option<u32>, Error> {
        let keychain = serde_json::to_string(&keychain)?;
        match self.select_last_derivation_index_by_keychain(keychain.clone())? {
            Some(value) => {
                self.delete_last_derivation_index_by_keychain(keychain)?;

                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    fn del_sync_time(&mut self) -> Result<Option<SyncTime>, Error> {
        match self.select_sync_time()? {
            Some(value) => {
                self.delete_sync_time()?;

                Ok(Some(value))
            }
            None => Ok(None),
        }
    }
}

impl Database for WalletDatabase {
    fn check_descriptor_checksum<B: AsRef<[u8]>>(
        &mut self,
        keychain: KeychainKind,
        bytes: B,
    ) -> Result<(), Error> {
        let keychain = serde_json::to_string(&keychain)?;

        match self.select_checksum_by_keychain(keychain.clone())? {
            Some(checksum) => {
                if checksum == bytes.as_ref().to_vec() {
                    Ok(())
                } else {
                    Err(Error::ChecksumMismatch)
                }
            }
            None => {
                self.insert_checksum(keychain, bytes.as_ref())?;
                Ok(())
            }
        }
    }

    fn iter_script_pubkeys(&self, keychain: Option<KeychainKind>) -> Result<Vec<Script>, Error> {
        match keychain {
            Some(keychain) => {
                let keychain = serde_json::to_string(&keychain)?;
                self.select_script_pubkeys_by_keychain(keychain)
            }
            None => self.select_script_pubkeys(),
        }
    }

    fn iter_utxos(&self) -> Result<Vec<LocalUtxo>, Error> {
        self.select_utxos()
    }

    fn iter_raw_txs(&self) -> Result<Vec<Transaction>, Error> {
        self.select_transactions()
    }

    fn iter_txs(&self, include_raw: bool) -> Result<Vec<TransactionDetails>, Error> {
        match include_raw {
            true => self.select_transaction_details_with_raw(),
            false => self.select_transaction_details(),
        }
    }

    fn get_script_pubkey_from_path(
        &self,
        keychain: KeychainKind,
        child: u32,
    ) -> Result<Option<Script>, Error> {
        let keychain = serde_json::to_string(&keychain)?;
        match self.select_script_pubkey_by_path(keychain, child)? {
            Some(script) => Ok(Some(script)),
            None => Ok(None),
        }
    }

    fn get_path_from_script_pubkey(
        &self,
        script: &Script,
    ) -> Result<Option<(KeychainKind, u32)>, Error> {
        match self.select_script_pubkey_by_script(script.as_bytes())? {
            Some((keychain, child)) => Ok(Some((keychain, child))),
            None => Ok(None),
        }
    }

    fn get_utxo(&self, outpoint: &OutPoint) -> Result<Option<LocalUtxo>, Error> {
        self.select_utxo_by_outpoint(&outpoint.txid, outpoint.vout)
    }

    fn get_raw_tx(&self, txid: &Txid) -> Result<Option<Transaction>, Error> {
        match self.select_transaction_by_txid(txid)? {
            Some(tx) => Ok(Some(tx)),
            None => Ok(None),
        }
    }

    fn get_tx(&self, txid: &Txid, include_raw: bool) -> Result<Option<TransactionDetails>, Error> {
        match self.select_transaction_details_by_txid(txid)? {
            Some(mut transaction_details) => {
                if !include_raw {
                    transaction_details.transaction = None;
                }
                Ok(Some(transaction_details))
            }
            None => Ok(None),
        }
    }

    fn get_last_index(&self, keychain: KeychainKind) -> Result<Option<u32>, Error> {
        let keychain = serde_json::to_string(&keychain)?;
        let value = self.select_last_derivation_index_by_keychain(keychain)?;
        Ok(value)
    }

    fn get_sync_time(&self) -> Result<Option<SyncTime>, Error> {
        self.select_sync_time()
    }

    fn increment_last_index(&mut self, keychain: KeychainKind) -> Result<u32, Error> {
        let keychain_string = serde_json::to_string(&keychain)?;
        match self.get_last_index(keychain)? {
            Some(value) => {
                self.update_last_derivation_index(keychain_string, value + 1)?;
                Ok(value + 1)
            }
            None => {
                self.insert_last_derivation_index(keychain_string, 0)?;
                Ok(0)
            }
        }
    }
}

impl BatchDatabase for WalletDatabase {
    type Batch = WalletDatabase;

    fn begin_batch(&self) -> Result<Self::Batch, Error> {
        tokio::task::block_in_place(move || {
            Handle::current().block_on(async move {
                let database = WalletDatabase {
                    settings: self.settings.clone(),
                    client: self
                        .client()
                        .await
                        .map_err(|e| Error::Generic(e.to_string()))?,
                };
                database
                    .client()
                    .await
                    .map_err(|e| Error::Generic(e.to_string()))?
                    .read()
                    .await
                    .batch_execute("BEGIN")
                    .await
                    .map_err(|e| {
                        Error::Generic(format!("Failed to begin SQL transaction: {}", e))
                    })?;
                Ok(database)
            })
        })
    }

    fn commit_batch(&mut self, batch: Self::Batch) -> Result<(), Error> {
        tokio::task::block_in_place(move || {
            Handle::current().block_on(async move {
                batch
                    .client()
                    .await
                    .map_err(|e| Error::Generic(e.to_string()))?
                    .read()
                    .await
                    .batch_execute("COMMIT")
                    .await
                    .map_err(|e| Error::Generic(format!("Failed to commit SQL transaction: {}", e)))
            })
        })
    }
}
