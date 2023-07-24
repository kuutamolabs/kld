use std::{
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::settings::{Network, Settings};
use anyhow::{bail, Result};
use async_trait::async_trait;
use bdk::{
    bitcoin::util::bip32::ExtendedPrivKey,
    bitcoincore_rpc::{bitcoincore_rpc_json::ScanningDetails, RpcApi},
    blockchain::{
        log_progress,
        rpc::{Auth, RpcSyncParams},
        ConfigurableBlockchain, RpcBlockchain, RpcConfig,
    },
    database::{BatchDatabase, BatchOperations, Database},
    wallet::AddressInfo,
    Balance, FeeRate, LocalUtxo, SignOptions, SyncOptions, TransactionDetails,
};
use bitcoin::{
    util::bip32::{ChildNumber, DerivationPath},
    Address, OutPoint, Script, Transaction,
};
use lightning::chain::chaininterface::{BroadcasterInterface, ConfirmationTarget, FeeEstimator};
use lightning_block_sync::BlockSource;
use log::{error, info, warn};
use once_cell::sync::OnceCell;

use crate::Service;

use super::WalletInterface;

pub struct Wallet<
    D: Database + BatchDatabase + BatchOperations,
    B: BlockSource + FeeEstimator + Service,
> {
    // bdk::Wallet uses a RefCell to hold the database which is not thread safe so we use a mutex here.
    wallet: Arc<Mutex<bdk::Wallet<D>>>,
    bitcoind_client: Arc<B>,
    settings: Arc<Settings>,
    blockchain: Arc<OnceCell<RpcBlockchain>>,
}

#[async_trait]
impl<
        D: Database + BatchDatabase + BatchOperations + Send + 'static,
        B: BlockSource + FeeEstimator + BroadcasterInterface + Service,
    > WalletInterface for Wallet<D, B>
{
    fn balance(&self) -> Result<Balance> {
        match self.wallet.try_lock() {
            Ok(wallet) => Ok(wallet.get_balance()?),
            Err(_) => {
                warn!("Wallet was locked when trying to get balance");
                Ok(Balance::default())
            }
        }
    }

    async fn transfer(
        &self,
        address: Address,
        amount: u64,
        fee_rate: Option<api::FeeRate>,
        min_conf: Option<u8>,
        utxos: Vec<OutPoint>,
    ) -> Result<(Transaction, TransactionDetails)> {
        if !self.bitcoind_client.is_synchronised().await {
            bail!("Bitcoind is syncronising the blockchain")
        }
        let height = match self.bitcoind_client.get_best_block().await {
            Ok((_, Some(height))) => height,
            _ => {
                bail!("Failed to fetch best block")
            }
        };

        match self.wallet.lock() {
            Ok(wallet) => {
                let mut tx_builder = wallet.build_tx();
                if amount == u64::MAX {
                    tx_builder.drain_wallet().drain_to(address.script_pubkey());
                } else {
                    tx_builder
                        .add_recipient(address.script_pubkey(), amount)
                        .drain_wallet()
                        .add_utxos(&utxos)?;
                };
                tx_builder.current_height(
                    min_conf.map_or_else(|| height, |min_conf| height - min_conf as u32),
                );
                if let Some(fee_rate) = fee_rate {
                    tx_builder.fee_rate(self.to_bdk_fee_rate(fee_rate));
                }
                let (mut psbt, tx_details) = tx_builder.finish()?;
                let _finalized = wallet.sign(&mut psbt, SignOptions::default())?;
                let tx = psbt.extract_tx();

                info!(
                    "Transferring {} sats to {address} with txid {}",
                    tx_details.sent, tx_details.txid
                );
                self.bitcoind_client.broadcast_transactions(&[&tx]);
                Ok((tx, tx_details))
            }
            Err(_) => bail!("Wallet is still syncing with chain"),
        }
    }

    fn new_address(&self) -> Result<AddressInfo> {
        let address = self
            .wallet
            .lock()
            .unwrap()
            .get_address(bdk::wallet::AddressIndex::LastUnused)?;
        Ok(address)
    }

    fn list_utxos(&self) -> Result<Vec<(LocalUtxo, TransactionDetails)>> {
        let mut result = vec![];
        match self.wallet.try_lock() {
            Ok(wallet) => {
                let utxos = wallet.list_unspent()?;
                for utxo in utxos {
                    if let Some(tx) = wallet.get_tx(&utxo.outpoint.txid, false)? {
                        result.push((utxo, tx));
                    }
                }
            }
            Err(_) => {
                warn!("Wallet was locked when trying to list utxos");
            }
        }
        Ok(result)
    }
}

impl<
        D: Database + BatchDatabase + BatchOperations + Send + 'static,
        B: BlockSource + FeeEstimator + Service,
    > Wallet<D, B>
{
    pub fn new(
        seed: &[u8; 32],
        settings: Arc<Settings>,
        bitcoind_client: Arc<B>,
        database: D,
    ) -> Result<Wallet<D, B>> {
        let xprivkey = ExtendedPrivKey::new_master(settings.bitcoin_network.into(), seed)?;
        let native_segwit_base_path = "m/84";

        let coin_type = match settings.bitcoin_network {
            Network::Main => 0,
            _ => 1,
        };

        let base_path = DerivationPath::from_str(native_segwit_base_path)?;
        let derivation_path = base_path.extend([ChildNumber::from_hardened_idx(coin_type)?]);
        let receive_descriptor_template = bdk::descriptor!(wpkh((
            xprivkey,
            derivation_path.extend([ChildNumber::Normal { index: 0 }])
        )))?;
        let change_descriptor_template = bdk::descriptor!(wpkh((
            xprivkey,
            derivation_path.extend([ChildNumber::Normal { index: 1 }])
        )))?;

        let bdk_wallet = Arc::new(Mutex::new(bdk::Wallet::new(
            receive_descriptor_template,
            Some(change_descriptor_template),
            settings.bitcoin_network.into(),
            database,
        )?));

        Ok(Wallet {
            wallet: bdk_wallet,
            bitcoind_client,
            settings,
            blockchain: Arc::new(OnceCell::new()),
        })
    }

    pub async fn synced(&self) -> bool {
        if let Ok((_, Some(height))) = self.bitcoind_client.get_best_block().await {
            if let Ok(wallet) = self.wallet.try_lock() {
                if let Ok(Some(sync_time)) = wallet.database().get_sync_time() {
                    return sync_time.block_time.height == height;
                }
            }
        }
        false
    }

    pub fn keep_sync_with_chain(&self) {
        let wallet_clone = self.wallet.clone();
        let blockchain = self.blockchain.clone();
        let settings = self.settings.clone();
        tokio::task::spawn_blocking(move || loop {
            // RPCBlockchain will not be instantiated if bitcoind is down. So within this loop we can keep trying to connect and get in sync.
            if let Ok(blockchain) =
                blockchain.get_or_try_init(|| init_rpc_blockchain(settings.clone()))
            {
                match blockchain.get_wallet_info() {
                    Ok(wallet_info) => match wallet_info.scanning {
                        Some(ScanningDetails::Scanning { duration, progress }) => {
                            info!(
                                    "Wallet is synchronising with the blockchain. {}% progress after {} seconds.",
                                    (progress * 100_f32).round(),
                                    duration
                                );
                        }
                        _ => {
                            if let Ok(info) = blockchain.get_blockchain_info() {
                                if let Ok(guard) = wallet_clone.try_lock() {
                                    let database = guard.database();
                                    if let Ok(synctime) = database.get_sync_time() {
                                        let sync_height = synctime
                                            .map(|time| time.block_time.height as u64)
                                            .unwrap_or_default();
                                        if sync_height < info.blocks {
                                            drop(database);
                                            info!("Starting wallet sync");
                                            if let Err(e) = guard.sync(
                                                blockchain,
                                                SyncOptions {
                                                    progress: Some(Box::new(log_progress())),
                                                },
                                            ) {
                                                error!("Wallet sync failed with bitcoind. Check the logs of your bitcoind for more context: {e:}");
                                            } else {
                                                info!("Wallet is synchronised to blockchain");
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    Err(e) => {
                        error!("Could not get wallet info: {e}");
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(10));
        });
    }

    pub fn fund_tx(
        &self,
        output_script: &Script,
        channel_value_satoshis: &u64,
        fee_rate: api::FeeRate,
    ) -> Result<Transaction> {
        let wallet = self.wallet.lock().unwrap();

        let mut tx_builder = wallet.build_tx();

        tx_builder
            .add_recipient(output_script.clone(), *channel_value_satoshis)
            .fee_rate(self.to_bdk_fee_rate(fee_rate))
            .enable_rbf();

        let (mut psbt, _tx_details) = tx_builder.finish()?;

        let _finalized = wallet.sign(&mut psbt, SignOptions::default())?;

        let funding_tx = psbt.extract_tx();
        Ok(funding_tx)
    }

    fn to_bdk_fee_rate(&self, fee_rate: api::FeeRate) -> FeeRate {
        match fee_rate {
            api::FeeRate::Urgent => FeeRate::from_sat_per_kwu(
                self.bitcoind_client
                    .get_est_sat_per_1000_weight(ConfirmationTarget::HighPriority)
                    as f32,
            ),
            api::FeeRate::Normal => FeeRate::from_sat_per_kwu(
                self.bitcoind_client
                    .get_est_sat_per_1000_weight(ConfirmationTarget::Normal) as f32,
            ),
            api::FeeRate::Slow => FeeRate::from_sat_per_kwu(
                self.bitcoind_client
                    .get_est_sat_per_1000_weight(ConfirmationTarget::Background)
                    as f32,
            ),
            api::FeeRate::PerKw(s) => FeeRate::from_sat_per_kwu(s as f32),
            api::FeeRate::PerKb(s) => FeeRate::from_sat_per_kvb(s as f32),
        }
    }
}

fn init_rpc_blockchain(settings: Arc<Settings>) -> Result<RpcBlockchain> {
    let start_time = if let Ok(time) = SystemTime::now().duration_since(UNIX_EPOCH) {
        time.as_secs()
    } else {
        error!("Incorrect system time");
        std::process::exit(1);
    };
    info!("Sync start time: {start_time}");
    // Sometimes we get wallet sync failure - https://github.com/bitcoindevkit/bdk/issues/859
    // It prevents a historical sync. So only add funds while kld is running.
    let rpc_sync_params = RpcSyncParams {
        start_script_count: 10,
        start_time,
        force_start_time: false,
        poll_rate_sec: 10,
    };

    let wallet_config = RpcConfig {
        url: format!(
            "http://{}:{}",
            settings.bitcoind_rpc_host, settings.bitcoind_rpc_port
        ),
        auth: Auth::Cookie {
            file: settings.bitcoin_cookie_path.clone().into(),
        },
        network: settings.bitcoin_network.into(),
        wallet_name: settings.wallet_name.clone(),
        sync_params: Some(rpc_sync_params),
    };
    Ok(RpcBlockchain::from_config(&wallet_config)?)
}

#[cfg(test)]
mod test {
    use std::{
        str::FromStr,
        sync::{Arc, Mutex},
    };

    use crate::settings::Settings;
    use anyhow::Result;
    use bdk::{database::MemoryDatabase, wallet::get_funded_wallet, Balance};
    use bitcoin::Address;
    use once_cell::sync::OnceCell;
    use test_utils::{TEST_ADDRESS, TEST_WPKH};

    use crate::{bitcoind::MockBitcoindClient, wallet::WalletInterface};

    use super::Wallet;

    #[test]
    fn test_fee_rate() -> Result<()> {
        let wallet = Wallet::new(
            &[0u8; 32],
            Arc::new(Settings::default()),
            Arc::new(MockBitcoindClient::default()),
            MemoryDatabase::new(),
        )?;

        let balance = wallet.balance()?;
        assert_eq!(balance, Balance::default());

        let urgent_fee_rate = wallet.to_bdk_fee_rate(api::FeeRate::Urgent);
        assert_eq!(40f32, urgent_fee_rate.as_sat_per_vb());

        let normal_fee_rate = wallet.to_bdk_fee_rate(api::FeeRate::Normal);
        assert_eq!(8f32, normal_fee_rate.as_sat_per_vb());

        let slow_fee_rate = wallet.to_bdk_fee_rate(api::FeeRate::Slow);
        assert_eq!(2f32, slow_fee_rate.as_sat_per_vb());

        let perkw_fee_rate = wallet.to_bdk_fee_rate(api::FeeRate::PerKw(4000));
        assert_eq!(16f32, perkw_fee_rate.as_sat_per_vb());

        let perkb_fee_rate = wallet.to_bdk_fee_rate(api::FeeRate::PerKb(1000));
        assert_eq!(1f32, perkb_fee_rate.as_sat_per_vb());

        Ok(())
    }

    #[tokio::test]
    async fn test_cannot_transfer_while_syncronising() -> Result<()> {
        let mut bitcoind_client = MockBitcoindClient::default();
        bitcoind_client.set_syncronised(false);
        let (bdk_wallet, _, _) = get_funded_wallet(TEST_WPKH);
        let bitcoind_client = Arc::new(bitcoind_client);
        let wallet = Wallet {
            bitcoind_client: bitcoind_client.clone(),
            wallet: Arc::new(Mutex::new(bdk_wallet)),
            settings: Arc::new(Settings::default()),
            blockchain: Arc::new(OnceCell::new()),
        };

        let res = wallet
            .transfer(
                Address::from_str(TEST_ADDRESS)?,
                u64::MAX,
                None,
                None,
                vec![],
            )
            .await;
        assert!(res.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn test_transfer() -> Result<()> {
        let bitcoind_client = MockBitcoindClient::default();
        let (bdk_wallet, _, _) = get_funded_wallet(TEST_WPKH);
        let bitcoind_client = Arc::new(bitcoind_client);
        let wallet = Wallet {
            bitcoind_client: bitcoind_client.clone(),
            wallet: Arc::new(Mutex::new(bdk_wallet)),
            settings: Arc::new(Settings::default()),
            blockchain: Arc::new(OnceCell::new()),
        };

        let (tx, tx_details) = wallet
            .transfer(
                Address::from_str(TEST_ADDRESS)?,
                u64::MAX,
                None,
                None,
                vec![],
            )
            .await?;

        assert!(!tx.input.is_empty());
        for input in &tx.input {
            assert!(!input.witness.is_empty());
        }
        assert!(!tx.output.is_empty());
        assert!(bitcoind_client.has_broadcast(tx_details.txid));

        Ok(())
    }
}
