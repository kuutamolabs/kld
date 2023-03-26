use std::{
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, bail, Context, Result};

use base64::{engine::general_purpose, Engine};
use bitcoin::{consensus::encode, Address, BlockHash, Transaction, Txid};
use bitcoincore_rpc_json::{EstimateMode, EstimateSmartFeeResult, GetBlockchainInfoResult};
use lightning::chain::chaininterface::{BroadcasterInterface, ConfirmationTarget, FeeEstimator};
use lightning_block_sync::{
    http::{HttpEndpoint, JsonResponse},
    rpc::RpcClient,
    AsyncBlockSourceResult, BlockData, BlockHeaderData, BlockSource,
};
use log::{error, info};
use serde::Deserialize;
use serde_json::{json, Value};
use settings::Settings;

use crate::{ldk::MIN_FEERATE, quit_signal};

pub struct BitcoindClient {
    client: Arc<RpcClient>,
    priorities: Arc<Priorities>,
}

impl BitcoindClient {
    pub async fn new(settings: &Settings) -> Result<BitcoindClient> {
        let cookie = std::fs::read(&settings.bitcoin_cookie_path)
            .context("Failed to read bitcoin cookie")?;
        let credentials = general_purpose::STANDARD.encode(cookie);
        let http_endpoint = HttpEndpoint::for_host(settings.bitcoind_rpc_host.clone())
            .with_port(settings.bitcoind_rpc_port);
        let client = Arc::new(
            RpcClient::new(&credentials, http_endpoint).context("failed to create rpc client")?,
        );

        let priorities = Arc::new(Priorities::new());
        let bitcoind_client = BitcoindClient { client, priorities };

        // Check that the bitcoind we've connected to is running the network we expect
        let bitcoind_chain = bitcoind_client.get_blockchain_info().await?.chain;
        if bitcoind_chain != settings.bitcoin_network.to_string() {
            bail!(
                "Chain argument ({}) didn't match bitcoind chain ({bitcoind_chain})",
                settings.bitcoin_network,
            );
        }
        Ok(bitcoind_client)
    }

    pub async fn wait_for_blockchain_synchronisation(&self) -> Result<()> {
        info!("Waiting for blockchain synchronisation.");
        let one_week = 60 * 60 * 24 * 7;
        let wait_for_shutdown = tokio::spawn(quit_signal());
        while !wait_for_shutdown.is_finished() {
            let info = self.get_blockchain_info().await?;
            let one_week_ago = SystemTime::now()
                .checked_sub(Duration::from_secs(one_week))
                .ok_or_else(|| anyhow!("wrong system time"))?
                .duration_since(UNIX_EPOCH)?
                .as_secs();

            if info.blocks == info.headers
                && info.median_time > one_week_ago
                // Its rare to see 100% verification.
                && info.verification_progress > 0.99
            {
                break;
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
        Ok(())
    }

    pub async fn send_transaction(&self, tx: &Transaction) -> Result<Txid> {
        let tx_serialized = json!(encode::serialize_hex(tx));
        BitcoindClient::send_transaction_with_client(self.client.clone(), tx_serialized).await
    }

    async fn send_transaction_with_client(
        client: Arc<RpcClient>,
        tx_serialized: Value,
    ) -> Result<Txid> {
        client
            .call_method::<JsonString>("sendrawtransaction", &[tx_serialized])
            .await?
            .deserialize()
    }

    pub async fn get_blockchain_info(&self) -> Result<GetBlockchainInfoResult> {
        self.client
            .call_method::<JsonString>("getblockchaininfo", &[])
            .await?
            .deserialize()
    }

    pub async fn generate_to_address(
        &self,
        n_blocks: u64,
        address: &Address,
    ) -> Result<Vec<BlockHash>> {
        self.client
            .call_method::<JsonString>("generatetoaddress", &[json!(n_blocks), json!(address)])
            .await?
            .deserialize()
    }

    pub async fn get_block_hash(&self, height: u32) -> Result<BlockHash> {
        self.client
            .call_method::<JsonString>("getblockhash", &[json!(height)])
            .await?
            .deserialize()
    }

    pub fn poll_for_fee_estimates(&self) {
        let client = self.client.clone();
        let priorities = self.priorities.clone();
        tokio::spawn(async move {
            loop {
                BitcoindClient::estimate_fee(
                    priorities.clone(),
                    client.clone(),
                    ConfirmationTarget::Background,
                )
                .await;
                BitcoindClient::estimate_fee(
                    priorities.clone(),
                    client.clone(),
                    ConfirmationTarget::Normal,
                )
                .await;
                BitcoindClient::estimate_fee(
                    priorities.clone(),
                    client.clone(),
                    ConfirmationTarget::HighPriority,
                )
                .await;
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });
    }

    async fn estimate_fee(
        priorities: Arc<Priorities>,
        client: Arc<RpcClient>,
        conf_target: ConfirmationTarget,
    ) {
        let priority = priorities.priority_of(&conf_target);
        match client
            .call_method::<JsonString>(
                "estimatesmartfee",
                &[json!(priority.n_blocks), json!(priority.estimate_mode)],
            )
            .await
            .map(|r| serde_json::from_str::<EstimateSmartFeeResult>(&r.0))
        {
            Ok(Ok(result)) => {
                // Bitcoind returns fee in BTC/kB.
                // So convert to sats and divide by 4 to get sats per 1000 weight units.
                let fee = ((result
                    .fee_rate
                    .map(|amount| amount.to_sat())
                    .unwrap_or(priority.default_fee_rate as u64)
                    / 4) as u32)
                    .max(MIN_FEERATE);
                priorities.store(conf_target, fee);
            }
            Ok(Err(e)) => error!("Could not fetch fee estimate: {}", e),
            Err(e) => error!("Could not fetch fee estimate: {}", e),
        };
    }
}

struct JsonString(String);

impl JsonString {
    fn deserialize<'a, T>(&'a self) -> Result<T>
    where
        T: Deserialize<'a>,
    {
        serde_json::from_str(&self.0).map_err(|e| anyhow!(e))
    }
}

impl TryInto<JsonString> for JsonResponse {
    type Error = std::io::Error;

    fn try_into(self) -> std::result::Result<JsonString, Self::Error> {
        Ok(JsonString(self.0.to_string()))
    }
}

impl FeeEstimator for BitcoindClient {
    fn get_est_sat_per_1000_weight(&self, confirmation_target: ConfirmationTarget) -> u32 {
        self.priorities.get(&confirmation_target)
    }
}

impl BroadcasterInterface for BitcoindClient {
    fn broadcast_transaction(&self, tx: &Transaction) {
        // This may error due to RL calling `broadcast_transaction` with the same transaction
        // multiple times, but the error is safe to ignore.
        let client = self.client.clone();
        let tx_serialized = json!(encode::serialize_hex(tx));
        tokio::spawn(async move {
            match BitcoindClient::send_transaction_with_client(client, tx_serialized).await {
                Ok(txid) => {
                    info!("Broadcast transaction {txid}");
                }
                Err(e) => {
                    let err_str = e.to_string();
                    if !err_str.contains("Transaction already in block chain")
                        && !err_str.contains("Inputs missing or spent")
                        && !err_str.contains("bad-txns-inputs-missingorspent")
                        && !err_str.contains("txn-mempool-conflict")
                        && !err_str.contains("non-BIP68-final")
                        && !err_str.contains("insufficient fee, rejecting replacement ")
                    {
                        error!("Broadcast transaction: {}", e);
                    }
                }
            }
        });
    }
}

impl BlockSource for BitcoindClient {
    fn get_header<'a>(
        &'a self,
        header_hash: &'a BlockHash,
        height_hint: Option<u32>,
    ) -> AsyncBlockSourceResult<'a, BlockHeaderData> {
        Box::pin(async move { self.client.get_header(header_hash, height_hint).await })
    }

    fn get_block<'a>(
        &'a self,
        header_hash: &'a BlockHash,
    ) -> AsyncBlockSourceResult<'a, BlockData> {
        Box::pin(async move { self.client.get_block(header_hash).await })
    }

    fn get_best_block(&self) -> AsyncBlockSourceResult<(BlockHash, Option<u32>)> {
        Box::pin(async move { self.client.get_best_block().await })
    }
}

struct Priority {
    // sats per 1000 weight unit
    fee_rate: AtomicU32,
    default_fee_rate: u32,
    n_blocks: u16,
    estimate_mode: EstimateMode,
}

struct Priorities {
    background: Arc<Priority>,
    normal: Arc<Priority>,
    high: Arc<Priority>,
}

impl Priorities {
    fn new() -> Priorities {
        Priorities {
            background: Arc::new(Priority {
                fee_rate: AtomicU32::new(MIN_FEERATE),
                default_fee_rate: MIN_FEERATE,
                n_blocks: 144,
                estimate_mode: EstimateMode::Economical,
            }),
            normal: Arc::new(Priority {
                fee_rate: AtomicU32::new(2000),
                default_fee_rate: 2000,
                n_blocks: 18,
                estimate_mode: EstimateMode::Economical,
            }),
            high: Arc::new(Priority {
                fee_rate: AtomicU32::new(5000),
                default_fee_rate: 5000,
                n_blocks: 6,
                estimate_mode: EstimateMode::Conservative,
            }),
        }
    }

    fn priority_of(&self, conf_target: &ConfirmationTarget) -> Arc<Priority> {
        match conf_target {
            ConfirmationTarget::Background => self.background.clone(),
            ConfirmationTarget::Normal => self.normal.clone(),
            ConfirmationTarget::HighPriority => self.high.clone(),
        }
    }

    fn get(&self, conf_target: &ConfirmationTarget) -> u32 {
        self.priority_of(conf_target)
            .fee_rate
            .load(Ordering::Acquire)
    }

    fn store(&self, conf_target: ConfirmationTarget, fee: u32) {
        self.priority_of(&conf_target)
            .fee_rate
            .store(fee, Ordering::Release);
    }
}
