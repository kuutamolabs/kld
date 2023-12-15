use std::{
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, bail, Context, Result};

use crate::settings::Settings;
use async_trait::async_trait;
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
use tokio::runtime::Handle;

use crate::{ldk::MIN_FEERATE, quit_signal, Service};

use super::bitcoind_interface::BitcoindInterface;

pub struct BitcoindClient {
    client: Arc<RpcClient>,
    priorities: Arc<Priorities>,
    handle: Handle,
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
        let bitcoind_client = BitcoindClient {
            client,
            priorities,
            handle: tokio::runtime::Handle::current(),
        };

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

    pub async fn wait_for_blockchain_synchronisation(&self) {
        info!("Waiting for blockchain synchronisation.");
        let wait_for_shutdown = tokio::spawn(quit_signal());
        while !wait_for_shutdown.is_finished() {
            if self.is_synchronised().await {
                info!("Blockchain is synchronised with network");
                return;
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
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
                BitcoindClient::estimate_fee(priorities.clone(), client.clone()).await;
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });
    }

    async fn estimate_fee(priorities: Arc<Priorities>, client: Arc<RpcClient>) {
        for class in priorities.list_class() {
            match client
                .call_method::<JsonString>(
                    "estimatesmartfee",
                    &[json!(class.n_blocks), json!(class.estimate_mode)],
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
                        .unwrap_or(class.default_fee_rate as u64)
                        / 4) as u32)
                        .max(MIN_FEERATE);
                    Priorities::store(class, fee);
                }
                Ok(Err(e)) => error!("Could not fetch fee estimate: {}", e),
                Err(e) => error!("Could not fetch fee estimate: {}", e),
            };
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct MempoolInfo {
    #[serde(rename = "mempoolminfee")]
    pub mempool_min_fee: f32,
}

#[async_trait]
impl BitcoindInterface for BitcoindClient {
    async fn get_blockchain_info(&self) -> Result<GetBlockchainInfoResult> {
        self.client
            .call_method::<JsonString>("getblockchaininfo", &[])
            .await?
            .deserialize()
    }

    async fn get_mempool_info(&self) -> Result<MempoolInfo> {
        self.client
            .call_method::<JsonString>("getmempoolinfo", &[])
            .await?
            .deserialize()
    }

    fn fee_rates_kw(&self) -> (u32, u32, u32) {
        let urgent = self.get_est_sat_per_1000_weight(ConfirmationTarget::OnChainSweep);
        let normal = self.get_est_sat_per_1000_weight(ConfirmationTarget::NonAnchorChannelFee);
        let slow = self.get_est_sat_per_1000_weight(ConfirmationTarget::ChannelCloseMinimum);
        (urgent, normal, slow)
    }

    async fn block_height(&self) -> Result<u64> {
        self.get_blockchain_info().await.map(|i| i.blocks)
    }
}

#[async_trait]
impl Service for BitcoindClient {
    async fn is_connected(&self) -> bool {
        self.get_best_block().await.is_ok()
    }

    async fn is_synchronised(&self) -> bool {
        let one_week = 60 * 60 * 24 * 7;
        let one_week_ago = SystemTime::now()
            .checked_sub(Duration::from_secs(one_week))
            .expect("wrong system time")
            .duration_since(UNIX_EPOCH)
            .expect("Wrong system time")
            .as_secs();
        match self.get_blockchain_info().await {
            Ok(info) => {
                info.blocks == info.headers
                    && info.median_time > one_week_ago
                    // Its rare to see 100% verification.
                    && info.verification_progress > 0.99
            }
            Err(_) => false,
        }
    }
}

#[async_trait]
pub trait BitcoindMetrics: Service {
    async fn block_height(&self) -> Result<u32>;
    fn fee_for(&self, target: ConfirmationTarget) -> u32;
}

#[async_trait]
impl BitcoindMetrics for BitcoindClient {
    async fn block_height(&self) -> Result<u32> {
        match self.client.get_best_block().await {
            Ok((_, Some(h))) => Ok(h),
            _ => Err(anyhow!("Could not get best block from bitcoind")),
        }
    }
    fn fee_for(&self, target: ConfirmationTarget) -> u32 {
        self.priorities.get(&target)
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
    fn broadcast_transactions(&self, txs: &[&Transaction]) {
        // This may error due to RL calling `broadcast_transaction` with the same transaction
        // multiple times, but the error is safe to ignore.
        let client = self.client.clone();
        for tx in txs {
            let tx_serialized = json!(encode::serialize_hex(tx));
            let client_cloned = client.clone();
            self.handle.spawn(async move {
                match BitcoindClient::send_transaction_with_client(client_cloned, tx_serialized)
                    .await
                {
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

struct PriorityClass {
    // sats per 1000 weight unit
    fee_rate: AtomicU32,
    default_fee_rate: u32,
    n_blocks: u16,
    estimate_mode: EstimateMode,
}

struct Priorities {
    background: Arc<PriorityClass>,
    normal: Arc<PriorityClass>,
    high: Arc<PriorityClass>,
}

impl Priorities {
    fn new() -> Priorities {
        Priorities {
            background: Arc::new(PriorityClass {
                fee_rate: AtomicU32::new(MIN_FEERATE),
                default_fee_rate: MIN_FEERATE,
                n_blocks: 144,
                estimate_mode: EstimateMode::Conservative,
            }),
            normal: Arc::new(PriorityClass {
                fee_rate: AtomicU32::new(5000),
                default_fee_rate: 5000,
                n_blocks: 18,
                estimate_mode: EstimateMode::Conservative,
            }),
            high: Arc::new(PriorityClass {
                fee_rate: AtomicU32::new(10000),
                default_fee_rate: 10000,
                n_blocks: 6,
                estimate_mode: EstimateMode::Economical,
            }),
        }
    }

    /// Return a base class and a mulipler
    fn priority_of(&self, conf_target: &ConfirmationTarget) -> (Arc<PriorityClass>, Option<f32>) {
        match conf_target {
            ConfirmationTarget::MaxAllowedNonAnchorChannelRemoteFee => {
                (self.high.clone(), Some(2.0))
            }
            ConfirmationTarget::OnChainSweep => (self.high.clone(), None),
            ConfirmationTarget::NonAnchorChannelFee => (self.normal.clone(), None),
            ConfirmationTarget::ChannelCloseMinimum
            | ConfirmationTarget::AnchorChannelFee
            | ConfirmationTarget::MinAllowedAnchorChannelRemoteFee
            | ConfirmationTarget::MinAllowedNonAnchorChannelRemoteFee => {
                (self.background.clone(), None)
            }
        }
    }

    fn get(&self, conf_target: &ConfirmationTarget) -> u32 {
        let (priority, multiplier) = self.priority_of(conf_target);
        let base = priority.fee_rate.load(Ordering::Acquire);
        if let Some(multiplier) = multiplier {
            ((base as f32) * multiplier) as u32
        } else {
            base
        }
    }

    fn store(class: Arc<PriorityClass>, fee: u32) {
        class.fee_rate.store(fee, Ordering::Release);
    }

    fn list_class(&self) -> Vec<Arc<PriorityClass>> {
        vec![
            self.background.clone(),
            self.normal.clone(),
            self.high.clone(),
        ]
    }
}
