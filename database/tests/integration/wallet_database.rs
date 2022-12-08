use std::str::FromStr;

use crate::with_cockroach;
use bdk::database::{BatchDatabase, BatchOperations, Database, SyncTime};
use bdk::{BlockTime, KeychainKind, LocalUtxo, TransactionDetails};
use bitcoin::consensus::encode::deserialize;
use bitcoin::hashes::hex::*;
use bitcoin::*;
use database::wallet_database::WalletDatabase;

#[tokio::test(flavor = "multi_thread")]
pub async fn test_script_pubkey() {
    with_cockroach(|settings| async move {
        let mut wallet_database = WalletDatabase::new(&settings).await.unwrap();
        let script = Script::from(
            Vec::<u8>::from_hex("76a91402306a7c23f3e8010de41e9e591348bb83f11daa88ac").unwrap(),
        );
        let path = 42;
        let keychain = KeychainKind::External;
        let mut batch = wallet_database.begin_batch();

        batch.set_script_pubkey(&script, keychain, path).unwrap();

        // Can't read while writes to the the same table are pending with cockroach.
        // assert_eq!(database.get_script_pubkey_from_path(keychain, path).unwrap(), None);
        // assert_eq!(database.get_path_from_script_pubkey(&script).unwrap(), None);

        wallet_database.commit_batch(batch).unwrap();

        assert_eq!(
            wallet_database
                .get_script_pubkey_from_path(keychain, path)
                .unwrap(),
            Some(script.clone())
        );
        assert_eq!(
            wallet_database
                .get_path_from_script_pubkey(&script)
                .unwrap(),
            Some((keychain, path))
        );

        assert_eq!(wallet_database.iter_script_pubkeys(None).unwrap().len(), 1);

        wallet_database
            .del_script_pubkey_from_path(keychain, path)
            .unwrap();
        assert_eq!(wallet_database.iter_script_pubkeys(None).unwrap().len(), 0);
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_utxo() {
    with_cockroach(|settings| async move {
        let mut wallet_database = WalletDatabase::new(&settings).await.unwrap();
        let outpoint = OutPoint::from_str(
            "5df6e0e2761359d30a8275058e299fcc0381534545f55cf43e41983f5d4c9456:0",
        )
        .unwrap();
        let script = Script::from(
            Vec::<u8>::from_hex("76a91402306a7c23f3e8010de41e9e591348bb83f11daa88ac").unwrap(),
        );
        let txout = TxOut {
            value: 133742,
            script_pubkey: script,
        };
        let utxo = LocalUtxo {
            txout,
            outpoint,
            keychain: KeychainKind::External,
            is_spent: true,
        };

        wallet_database.set_utxo(&utxo).unwrap();
        wallet_database.set_utxo(&utxo).unwrap();
        assert_eq!(wallet_database.iter_utxos().unwrap().len(), 1);
        assert_eq!(wallet_database.get_utxo(&outpoint).unwrap(), Some(utxo));
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_raw_tx() {
    with_cockroach(|settings| async move {
    let mut wallet_database = WalletDatabase::new(&settings).await.unwrap();
    let hex_tx = Vec::<u8>::from_hex("0100000001a15d57094aa7a21a28cb20b59aab8fc7d1149a3bdbcddba9c622e4f5f6a99ece010000006c493046022100f93bb0e7d8db7bd46e40132d1f8242026e045f03a0efe71bbb8e3f475e970d790221009337cd7f1f929f00cc6ff01f03729b069a7c21b59b1736ddfee5db5946c5da8c0121033b9b137ee87d5a812d6f506efdd37f0affa7ffc310711c06c7f3e097c9447c52ffffffff0100e1f505000000001976a9140389035a9225b3839e2bbf32d826a1e222031fd888ac00000000").unwrap();
    let tx: Transaction = deserialize(&hex_tx).unwrap();

    wallet_database.set_raw_tx(&tx).unwrap();

    let txid = tx.txid();

    assert_eq!(wallet_database.get_raw_tx(&txid).unwrap(), Some(tx));
    }).await;
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_tx() {
    with_cockroach(|settings| async move {
    let mut wallet_database = WalletDatabase::new(&settings).await.unwrap();
    let hex_tx = Vec::<u8>::from_hex("0100000001a15d57094aa7a21a28cb20b59aab8fc7d1149a3bdbcddba9c622e4f5f6a99ece010000006c493046022100f93bb0e7d8db7bd46e40132d1f8242026e045f03a0efe71bbb8e3f475e970d790221009337cd7f1f929f00cc6ff01f03729b069a7c21b59b1736ddfee5db5946c5da8c0121033b9b137ee87d5a812d6f506efdd37f0affa7ffc310711c06c7f3e097c9447c52ffffffff0100e1f505000000001976a9140389035a9225b3839e2bbf32d826a1e222031fd888ac00000000").unwrap();
    let tx: Transaction = deserialize(&hex_tx).unwrap();
    let txid = tx.txid();
    let mut tx_details = TransactionDetails {
        transaction: Some(tx),
        txid,
        received: 1337,
        sent: 420420,
        fee: Some(140),
        confirmation_time: Some(BlockTime {
            timestamp: 123456,
            height: 1000,
        }),
    };

    wallet_database.set_tx(&tx_details).unwrap();

    // get with raw tx too
    assert_eq!(
        wallet_database.get_tx(&tx_details.txid, true).unwrap(),
        Some(tx_details.clone())
    );
    // get only raw_tx
    assert_eq!(
        wallet_database.get_raw_tx(&tx_details.txid).unwrap(),
        tx_details.transaction
    );

    // now get without raw_tx
    tx_details.transaction = None;
    assert_eq!(
        wallet_database.get_tx(&tx_details.txid, false).unwrap(),
        Some(tx_details)
    );
    }).await;
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_last_index() {
    with_cockroach(|settings| async move {
        let mut wallet_database = WalletDatabase::new(&settings).await.unwrap();
        wallet_database
            .set_last_index(KeychainKind::External, 1337)
            .unwrap();

        assert_eq!(
            wallet_database
                .get_last_index(KeychainKind::External)
                .unwrap(),
            Some(1337)
        );
        assert_eq!(
            wallet_database
                .get_last_index(KeychainKind::Internal)
                .unwrap(),
            None
        );

        let res = wallet_database
            .increment_last_index(KeychainKind::External)
            .unwrap();
        assert_eq!(res, 1338);
        let res = wallet_database
            .increment_last_index(KeychainKind::Internal)
            .unwrap();
        assert_eq!(res, 0);

        assert_eq!(
            wallet_database
                .get_last_index(KeychainKind::External)
                .unwrap(),
            Some(1338)
        );
        assert_eq!(
            wallet_database
                .get_last_index(KeychainKind::Internal)
                .unwrap(),
            Some(0)
        );
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_sync_time() {
    with_cockroach(|settings| async move {
        let mut wallet_database = WalletDatabase::new(&settings).await.unwrap();
        assert!(wallet_database.get_sync_time().unwrap().is_none());

        wallet_database
            .set_sync_time(SyncTime {
                block_time: BlockTime {
                    height: 100,
                    timestamp: 1000,
                },
            })
            .unwrap();

        let extracted = wallet_database.get_sync_time().unwrap();
        assert!(extracted.is_some());
        assert_eq!(extracted.as_ref().unwrap().block_time.height, 100);
        assert_eq!(extracted.as_ref().unwrap().block_time.timestamp, 1000);

        wallet_database.del_sync_time().unwrap();
        assert!(wallet_database.get_sync_time().unwrap().is_none());
    })
    .await;
}
