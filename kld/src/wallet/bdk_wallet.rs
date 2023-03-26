use std::{
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Result};
use async_trait::async_trait;
use bdk::{
    bitcoin::util::bip32::ExtendedPrivKey,
    bitcoincore_rpc::{bitcoincore_rpc_json::ScanningDetails, RpcApi},
    blockchain::{
        rpc::{Auth, RpcSyncParams},
        ConfigurableBlockchain, RpcBlockchain, RpcConfig,
    },
    database::{BatchDatabase, BatchOperations, Database},
    wallet::AddressInfo,
    Balance, FeeRate, SignOptions, SyncOptions,
};
use bitcoin::{
    util::bip32::{ChildNumber, DerivationPath},
    Address, OutPoint, Script, Transaction,
};
use lightning::chain::chaininterface::{ConfirmationTarget, FeeEstimator};
use lightning_block_sync::BlockSource;
use log::{error, info};
use settings::{Network, Settings};

use super::WalletInterface;

pub struct Wallet<D: Database + BatchDatabase + BatchOperations, B: BlockSource + FeeEstimator> {
    // bdk::Wallet uses a RefCell to hold the database which is not thread safe so we use a mutex here.
    wallet: Arc<Mutex<bdk::Wallet<D>>>,
    bitcoind_client: Arc<B>,
    settings: Arc<Settings>,
}

#[async_trait]
impl<
        D: Database + BatchDatabase + BatchOperations + Send + 'static,
        B: BlockSource + FeeEstimator,
    > WalletInterface for Wallet<D, B>
{
    fn balance(&self) -> Result<Balance> {
        match self.wallet.try_lock() {
            Ok(wallet) => Ok(wallet.get_balance()?),
            Err(_) => Ok(Balance::default()),
        }
    }

    async fn transfer(
        &self,
        address: Address,
        amount: u64,
        fee_rate: Option<api::FeeRate>,
        min_conf: Option<u8>,
        utxos: Vec<OutPoint>,
    ) -> Result<Transaction> {
        let height = match self.bitcoind_client.get_best_block().await {
            Ok((_, Some(height))) => height,
            _ => {
                bail!("Failed to fetch best block")
            }
        };

        match self.wallet.try_lock() {
            Ok(wallet) => {
                let mut tx_builder = wallet.build_tx();
                if amount == u64::MAX {
                    tx_builder.drain_wallet().drain_to(address.script_pubkey());
                } else {
                    tx_builder
                        .add_recipient(address.script_pubkey(), amount)
                        .drain_wallet()
                        .add_utxos(&utxos)?;
                }
                tx_builder.current_height(
                    min_conf.map_or_else(|| height, |min_conf| height - min_conf as u32),
                );
                if let Some(fee_rate) = fee_rate {
                    tx_builder.fee_rate(self.to_bdk_fee_rate(fee_rate));
                }
                let tx = tx_builder.finish()?.0.extract_tx();
                Ok(tx)
            }
            Err(_) => bail!("Wallet is still syncing with chain"),
        }
    }

    fn new_address(&self) -> Result<AddressInfo> {
        let address = self
            .wallet
            .try_lock()
            .unwrap()
            .get_address(bdk::wallet::AddressIndex::LastUnused)?;
        Ok(address)
    }
}

impl<
        D: Database + BatchDatabase + BatchOperations + Send + 'static,
        B: BlockSource + FeeEstimator,
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
        })
    }

    pub fn keep_sync_with_chain(&self) -> Result<()> {
        let url = format!(
            "http://{}:{}",
            self.settings.bitcoind_rpc_host, self.settings.bitcoind_rpc_port
        );

        // Sometimes we get wallet sync failure - https://github.com/bitcoindevkit/bdk/issues/859
        // It prevents a historical sync. So only add funds while kld is running.
        let rpc_sync_params = RpcSyncParams {
            start_script_count: 100,
            start_time: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            force_start_time: false,
            poll_rate_sec: 10,
        };

        let wallet_config = RpcConfig {
            url: url.clone(),
            auth: Auth::Cookie {
                file: self.settings.bitcoin_cookie_path.clone().into(),
            },
            network: self.settings.bitcoin_network.into(),
            wallet_name: "kld-wallet".to_string(),
            sync_params: Some(rpc_sync_params),
        };
        let blockchain = RpcBlockchain::from_config(&wallet_config)?;

        let wallet_clone = self.wallet.clone();
        tokio::task::spawn_blocking(move || {
            loop {
                match blockchain.get_wallet_info() {
                    Ok(wallet_info) => {
                        match wallet_info.scanning {
                            Some(ScanningDetails::Scanning { duration, progress }) => {
                                info!(
                                    "Wallet is synchronising with the blockchain. {}% progress after {} seconds.",
                                    (progress * 100_f32).round(),
                                    duration
                                );
                            }
                            _ => {
                                // Don't want to block for a long time while the wallet is syncing so use try_lock everywhere else.
                                if let Err(e) = wallet_clone
                                    .lock()
                                    .expect("Cannot obtain mutex for wallet")
                                    .sync(&blockchain, SyncOptions::default())
                                {
                                    error!("Walled sync failed with bitcoind rpc endpoint {url:}. Check the logs of your bitcoind for more context: {e:}");
                                } else {
                                    info!("Wallet is synchronised to blockchain");
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Could not get wallet info: {e}");
                    }
                }
                std::thread::sleep(Duration::from_secs(60));
            }
        });
        Ok(())
    }

    pub fn fund_tx(
        &self,
        output_script: &Script,
        channel_value_satoshis: &u64,
        fee_rate: api::FeeRate,
    ) -> Result<Transaction> {
        let wallet = self.wallet.try_lock().unwrap();

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

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use anyhow::Result;
    use bdk::{database::MemoryDatabase, Balance};
    use settings::Settings;

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
}
