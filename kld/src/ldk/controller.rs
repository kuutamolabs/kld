use crate::bitcoind::bitcoind_interface::BitcoindInterface;
use crate::bitcoind::{BitcoindClient, BitcoindUtxoLookup};
use crate::database::channel::Channel;
use crate::database::forward::{Forward, ForwardStatus, TotalForwards};
use crate::database::invoice::Invoice;
use crate::database::payment::{Payment, PaymentDirection};
use crate::wallet::{Wallet, WalletInterface};
use crate::{log_error, MillisatAmount, Service};

use crate::api::SocketAddress;
use crate::database::{DurableConnection, LdkDatabase, WalletDatabase};
use anyhow::{anyhow, bail, Context, Result};
use api::FeeRate;
use async_trait::async_trait;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use bitcoin::{BlockHash, Network, Transaction};
use lightning::chain;
use lightning::chain::channelmonitor::ChannelMonitor;
use lightning::chain::BestBlock;
use lightning::chain::Watch;
use lightning::ln::channelmanager::{
    self, ChannelDetails, PaymentId, PaymentSendFailure, RecipientOnionFields,
};
use lightning::ln::channelmanager::{ChainParameters, ChannelManagerReadArgs};
use lightning::ln::peer_handler::{IgnoringMessageHandler, MessageHandler};
use lightning::ln::ChannelId;
use lightning::routing::gossip::{ChannelInfo, NodeId, NodeInfo, P2PGossipSync};
use lightning::routing::router::{DefaultRouter, PaymentParameters, RouteParameters, Router};
use lightning::routing::scoring::{
    ProbabilisticScorer, ProbabilisticScoringDecayParameters, ProbabilisticScoringFeeParameters,
};
use lightning::sign::{InMemorySigner, KeysManager};
use lightning::util::config::UserConfig;
use lightning::util::errors::APIError;

use crate::ldk::peer_manager::KuutamoPeerManger;
use crate::logger::KldLogger;
use crate::settings::Settings;
use ldk_lsp_client::LiquidityProviderConfig;
use lightning::util::indexed_map::IndexedMap;
use lightning_background_processor::{process_events_async, GossipSync};
use lightning_block_sync::poll;
use lightning_block_sync::SpvClient;
use lightning_block_sync::UnboundedCache;
use lightning_block_sync::{init, BlockSourceResult};
use lightning_invoice::DEFAULT_EXPIRY_TIME;
use log::{error, info, warn};
use rand::random;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use futures::{future::Shared, Future};
use tokio::sync::oneshot::{self, Receiver, Sender};
use tokio::sync::RwLock;

use super::event_handler::EventHandler;
use super::peer_manager::PeerManager;
use super::{
    ldk_error, lightning_error, payment_send_failure, retryable_send_failure,
    sign_or_creation_error, ChainMonitor, ChannelManager, KldRouter, LightningInterface,
    LiquidityManager, NetworkGraph, OnionMessenger, OpenChannelResult, Peer, PeerStatus, Scorer,
};

#[async_trait]
impl LightningInterface for Controller {
    fn identity_pubkey(&self) -> PublicKey {
        self.channel_manager.get_our_node_id()
    }

    async fn synced(&self) -> Result<bool> {
        Ok(self.bitcoind_client.is_synchronised().await && self.wallet.synced().await)
    }

    fn sign(&self, message: &[u8]) -> Result<String> {
        let secret_key = self.keys_manager.get_node_secret_key();
        let signature = lightning::util::message_signing::sign(message, &secret_key)?;
        Ok(signature)
    }

    fn graph_num_nodes(&self) -> usize {
        self.network_graph.read_only().nodes().len()
    }

    fn graph_num_channels(&self) -> usize {
        self.network_graph.read_only().channels().len()
    }

    fn num_peers(&self) -> usize {
        self.peer_manager.get_connected_peers().len()
    }

    fn wallet_balance(&self) -> u64 {
        match self.wallet.balance() {
            Ok(balance) => balance.confirmed,
            Err(e) => {
                error!("Unable to get wallet balance for metrics: {}", e);
                0
            }
        }
    }

    fn alias(&self) -> String {
        self.settings.node_alias.clone()
    }

    fn color(&self) -> String {
        self.settings.node_alias_color.clone()
    }

    fn network(&self) -> bitcoin::Network {
        self.settings.bitcoin_network.into()
    }

    fn num_active_channels(&self) -> usize {
        self.channel_manager
            .list_channels()
            .iter()
            .filter(|c| c.is_usable)
            .count()
    }

    fn num_inactive_channels(&self) -> usize {
        self.channel_manager
            .list_channels()
            .iter()
            .filter(|c| c.is_channel_ready && !c.is_usable)
            .count()
    }

    fn num_pending_channels(&self) -> usize {
        self.channel_manager
            .list_channels()
            .iter()
            .filter(|c| !c.is_channel_ready)
            .count()
    }

    fn list_channels(&self) -> Vec<ChannelDetails> {
        self.channel_manager.list_channels()
    }

    async fn open_channel(
        &self,
        their_network_key: PublicKey,
        channel_value_satoshis: u64,
        push_msat: Option<u64>,
        fee_rate: Option<FeeRate>,
        override_config: Option<UserConfig>,
    ) -> Result<OpenChannelResult> {
        if !self.bitcoind_client.is_synchronised().await {
            bail!("Bitcoind is synchronising blockchain")
        }
        if !self.peer_manager.is_connected(&their_network_key) {
            return Err(anyhow!("Peer not connected"));
        }
        let user_channel_id: u64 = random::<u64>() / 2; // To fit into the database INT
        let channel_id = self
            .channel_manager
            .create_channel(
                their_network_key,
                channel_value_satoshis,
                push_msat.unwrap_or_default(),
                user_channel_id as u128,
                override_config,
            )
            .map_err(ldk_error)?;
        let receiver = self
            .async_api_requests
            .funding_transactions
            .insert(user_channel_id, fee_rate.unwrap_or_default())
            .await;
        let transaction = receiver.await??;
        let txid = transaction.txid();
        Ok(OpenChannelResult {
            transaction,
            txid,
            channel_id,
        })
    }

    async fn close_channel(
        &self,
        channel_id: &ChannelId,
        counterparty_node_id: &PublicKey,
    ) -> Result<()> {
        if !self.bitcoind_client.is_synchronised().await {
            bail!("Bitcoind is synchronising blockchain")
        }
        self.channel_manager
            .close_channel(channel_id, counterparty_node_id)
            .map_err(ldk_error)
    }

    fn set_channel_fee(
        &self,
        counterparty_node_id: &PublicKey,
        channel_ids: &[ChannelId],
        forwarding_fee_proportional_millionths: Option<u32>,
        forwarding_fee_base_msat: Option<u32>,
    ) -> Result<(u32, u32)> {
        let mut channel_config = self.user_config().channel_config;
        if let Some(fee) = forwarding_fee_proportional_millionths {
            channel_config.forwarding_fee_proportional_millionths = fee;
        }
        if let Some(fee) = forwarding_fee_base_msat {
            channel_config.forwarding_fee_base_msat = fee;
        }
        self.channel_manager
            .update_channel_config(counterparty_node_id, channel_ids, &channel_config)
            .map_err(ldk_error)?;
        Ok((
            channel_config.forwarding_fee_base_msat,
            channel_config.forwarding_fee_proportional_millionths,
        ))
    }

    fn alias_of(&self, public_key: &PublicKey) -> Option<String> {
        self.network_graph
            .read_only()
            .node(&NodeId::from_pubkey(public_key))
            .and_then(|n| n.announcement_info.as_ref().map(|a| a.alias.to_string()))
    }

    /// List all the peers that we have channels with along with their connection status.
    async fn list_peers(&self) -> Result<Vec<Peer>> {
        let connected_peers = self.peer_manager.get_connected_peers();
        let channel_peers: Vec<PublicKey> = self
            .channel_manager
            .list_channels()
            .iter()
            .map(|c| c.counterparty.node_id)
            .collect();
        let persistent_peers = self.database.fetch_peers().await?;

        let mut response = vec![];

        let mut all_pub_keys: HashSet<PublicKey> = HashSet::from_iter(
            connected_peers
                .iter()
                .map(|p| p.0)
                .collect::<Vec<PublicKey>>(),
        );
        all_pub_keys.extend(channel_peers);
        all_pub_keys.extend(persistent_peers.keys());

        for public_key in all_pub_keys {
            let net_address = connected_peers
                .iter()
                .find(|p| p.0 == public_key)
                .and_then(|p| p.1.clone());
            let status = if net_address.is_some() {
                PeerStatus::Connected
            } else {
                PeerStatus::Disconnected
            };
            response.push(Peer {
                public_key,
                net_address,
                status,
                alias: self.alias_of(&public_key).unwrap_or_default(),
            });
        }
        Ok(response)
    }

    async fn connect_peer(
        &self,
        public_key: PublicKey,
        peer_address: Option<SocketAddress>,
    ) -> Result<()> {
        if let Some(net_address) = peer_address {
            self.peer_manager
                .connect_peer(self.database.clone(), public_key, net_address)
                .await
        } else {
            let addresses: Vec<SocketAddress> = self
                .network_graph
                .read_only()
                .get_addresses(&public_key)
                .context("No addresses found for node")?
                .into_iter()
                .map(|a| a.into())
                .filter(|a: &SocketAddress| a.is_ipv4())
                .collect();
            for address in addresses {
                if let Err(e) = self
                    .peer_manager
                    .connect_peer(self.database.clone(), public_key, address.clone())
                    .await
                {
                    info!("Could not connect to {public_key}@{address}. {}", e);
                } else {
                    return Ok(());
                }
            }
            Err(anyhow!("Could not connect to any peer addresses."))
        }
    }

    async fn disconnect_peer(&self, public_key: PublicKey) -> Result<()> {
        self.peer_manager
            .disconnect_and_drop_by_node_id(self.database.clone(), public_key)
            .await
    }

    fn public_addresses(&self) -> Vec<SocketAddress> {
        self.settings.public_addresses.clone()
    }

    fn get_node(&self, node_id: &NodeId) -> Option<NodeInfo> {
        self.network_graph.read_only().node(node_id).cloned()
    }

    fn nodes(&self) -> IndexedMap<NodeId, NodeInfo> {
        self.network_graph.read_only().nodes().clone()
    }

    fn get_channel(&self, channel_id: u64) -> Option<ChannelInfo> {
        self.network_graph.read_only().channel(channel_id).cloned()
    }

    fn channels(&self) -> IndexedMap<u64, ChannelInfo> {
        self.network_graph.read_only().channels().clone()
    }

    // Use this to override the default/startup config.
    fn user_config(&self) -> UserConfig {
        *self.channel_manager.get_current_default_configuration()
    }

    async fn generate_invoice(
        &self,
        label: String,
        amount: Option<u64>,
        description: String,
        expiry: Option<u32>,
    ) -> Result<Invoice> {
        let bolt11 = lightning_invoice::utils::create_invoice_from_channelmanager(
            &self.channel_manager,
            self.keys_manager.clone(),
            KldLogger::global(),
            self.network().into(),
            amount,
            description,
            expiry.unwrap_or(DEFAULT_EXPIRY_TIME as u32),
            None,
        )
        .map_err(sign_or_creation_error)?;
        let invoice = Invoice::new(Some(label), bolt11)?;
        info!(
            "Generated invoice with payment hash {}",
            invoice.payment_hash.0.to_hex()
        );
        self.database.persist_invoice(&invoice).await?;
        Ok(invoice)
    }

    async fn list_invoices(&self, label: Option<String>) -> Result<Vec<Invoice>> {
        self.database.fetch_invoices(label).await
    }

    async fn pay_invoice(&self, invoice: Invoice, label: Option<String>) -> Result<Payment> {
        let payment = Payment::of_invoice_outbound(&invoice, label);

        let route_params = RouteParameters {
            payment_params: PaymentParameters::from_node_id(invoice.payee_pub_key, 40),
            final_value_msat: invoice.amount.context("amount missing from invoice")?,
            // TODO: configurable, when opening a channel or starting kld
            max_total_routing_fee_msat: None,
        };
        self.channel_manager
            .send_payment(
                payment.hash.context("expected payment hash")?,
                RecipientOnionFields::secret_only(*invoice.bolt11.payment_secret()),
                payment.id,
                route_params,
                channelmanager::Retry::Timeout(Duration::from_secs(60)),
            )
            .map_err(retryable_send_failure)?;
        info!(
            "Initiated payment of invoice with hash {}",
            invoice.payment_hash.0.to_hex()
        );
        self.database.persist_invoice(&invoice).await?;
        self.database.persist_payment(&payment).await?;
        let receiver = self
            .async_api_requests
            .payments
            .insert(payment.id, payment)
            .await;
        let payment = receiver.await??;
        self.database.persist_payment(&payment).await?;
        Ok(payment)
    }

    async fn keysend_payment(&self, payee: NodeId, amount: MillisatAmount) -> Result<Payment> {
        let payment_id = Payment::new_id();
        let inflight_htlcs = self.channel_manager.compute_inflight_htlcs();
        let route_params = RouteParameters {
            payment_params: PaymentParameters::for_keysend(payee.as_pubkey()?, 40, false),
            final_value_msat: amount,
            // TODO: configurable, when opening a channel or starting kld
            max_total_routing_fee_msat: None,
        };
        let route = self
            .router
            .find_route(&self.identity_pubkey(), &route_params, None, inflight_htlcs)
            .map_err(lightning_error)?;
        match self.channel_manager.send_spontaneous_payment(
            &route,
            None,
            RecipientOnionFields::spontaneous_empty(),
            payment_id,
        ) {
            Ok(_hash) => (),
            Err(e) => {
                match &e {
                    PaymentSendFailure::PartialFailure {
                        results,
                        failed_paths_retry: _,
                        payment_id: _,
                    } => {
                        // Monitor updates are persisted async so continue if MonitorUpdateInProgress is the only "error" we get.
                        if !results.iter().all(|result| {
                            result.is_ok()
                                || result
                                    .as_ref()
                                    .is_err_and(|f| matches!(f, APIError::MonitorUpdateInProgress))
                        }) {
                            return Err(payment_send_failure(e));
                        }
                    }
                    _ => return Err(payment_send_failure(e)),
                };
            }
        };
        let payment = Payment::spontaneous_outbound(payment_id, amount);
        info!(
            "Initiated keysend payment with id {}",
            payment_id.0.to_hex()
        );
        self.database.persist_payment(&payment).await?;
        let receiver = self
            .async_api_requests
            .payments
            .insert(payment_id, payment)
            .await;
        let payment = receiver.await??;
        self.database.persist_payment(&payment).await?;
        Ok(payment)
    }

    async fn list_payments(
        &self,
        invoice: Option<Invoice>,
        direction: Option<PaymentDirection>,
    ) -> Result<Vec<Payment>> {
        self.database
            .fetch_payments(invoice.map(|i| i.payment_hash), direction)
            .await
    }

    async fn estimated_channel_liquidity_range(
        &self,
        scid: u64,
        target: &NodeId,
    ) -> Result<Option<(u64, u64)>> {
        Ok(self
            .scorer
            .try_read()
            .map_err(|e| anyhow!("failed to acquire lock on scorer {}", e))?
            .estimated_channel_liquidity_range(scid, target))
    }

    async fn fetch_total_forwards(&self) -> Result<TotalForwards> {
        self.database.fetch_total_forwards().await
    }

    async fn fetch_forwards(&self, status: Option<ForwardStatus>) -> Result<Vec<Forward>> {
        self.database.fetch_forwards(status).await
    }

    async fn channel_history(&self) -> Result<Vec<Channel>> {
        self.database.fetch_channel_history().await
    }
}

pub(crate) struct AsyncAPIRequests {
    pub funding_transactions: AsyncSenders<u64, FeeRate, Result<Transaction>>,
    pub payments: AsyncSenders<PaymentId, Payment, Result<Payment>>,
}

impl AsyncAPIRequests {
    fn new() -> AsyncAPIRequests {
        AsyncAPIRequests {
            funding_transactions: AsyncSenders::new(),
            payments: AsyncSenders::new(),
        }
    }
}

pub(crate) struct AsyncSenders<K, V, RV> {
    senders: RwLock<HashMap<K, (V, Sender<RV>)>>,
}

impl<K: Eq + std::hash::Hash, V: Clone, RV> AsyncSenders<K, V, RV> {
    fn new() -> AsyncSenders<K, V, RV> {
        AsyncSenders {
            senders: RwLock::new(HashMap::new()),
        }
    }

    async fn insert(&self, k: K, v: V) -> Receiver<RV> {
        let (tx, rx) = oneshot::channel::<RV>();
        self.senders.write().await.insert(k, (v, tx));
        rx
    }

    pub async fn get(&self, k: &K) -> Option<(V, impl FnOnce(RV))> {
        if let Some((v, tx)) = self.senders.write().await.remove(k) {
            let respond = |rv: RV| {
                if tx.send(rv).is_err() {
                    warn!("Receiver dropped");
                }
            };
            return Some((v, respond));
        }
        None
    }

    pub async fn respond(&self, k: &K, rv: RV) {
        if let Some((_, tx)) = self.senders.write().await.remove(k) {
            if tx.send(rv).is_err() {
                warn!("Receiver dropped");
            }
        }
    }
}

pub struct Controller {
    settings: Arc<Settings>,
    database: Arc<LdkDatabase>,
    bitcoind_client: Arc<BitcoindClient>,
    channel_manager: Arc<ChannelManager>,
    peer_manager: Arc<PeerManager>,
    keys_manager: Arc<KeysManager>,
    network_graph: Arc<NetworkGraph>,
    router: Arc<KldRouter>,
    scorer: Arc<std::sync::RwLock<Scorer>>,
    wallet: Arc<Wallet<WalletDatabase, BitcoindClient>>,
    async_api_requests: Arc<AsyncAPIRequests>,
    _liquidity_manager: Arc<LiquidityManager>,
}

impl Controller {
    pub fn stop(&self) {
        // Disconnect our peers and stop accepting new connections. This ensures we don't continue
        // updating our channel data after we've stopped the background processor.
        self.peer_manager.disconnect_all_peers();
    }

    pub async fn start_ldk(
        settings: Arc<Settings>,
        durable_connection: Arc<DurableConnection>,
        bitcoind_client: Arc<BitcoindClient>,
        wallet: Arc<Wallet<WalletDatabase, BitcoindClient>>,
        seed: &[u8; 32],
        quit_signal: Shared<impl Future<Output = ()> + Send + 'static>,
    ) -> Result<Controller> {
        let database = Arc::new(LdkDatabase::new(
            settings.clone(),
            durable_connection.clone(),
        ));

        // BitcoindClient implements the FeeEstimator trait, so it'll act as our fee estimator.
        let fee_estimator = bitcoind_client.clone();

        // BitcoindClient implements the BroadcasterInterface trait, so it'll act as our transaction broadcaster.
        let broadcaster = bitcoind_client.clone();

        let network = settings.bitcoin_network.into();

        let chain_monitor: Arc<ChainMonitor> = Arc::new(ChainMonitor::new(
            None,
            broadcaster.clone(),
            KldLogger::global(),
            fee_estimator.clone(),
            database.clone(),
        ));
        database.set_chain_monitor(chain_monitor.clone());

        let is_first_start = database
            .is_first_start()
            .await
            .context("could not check if database has been initialized")?;
        // Initialize the KeysManager
        // The key seed that we use to derive the node privkey (that corresponds to the node pubkey) and
        // other secret key material.
        let current_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let keys_manager = Arc::new(KeysManager::new(
            seed,
            current_time.as_secs(),
            current_time.subsec_nanos(),
        ));

        let network_graph = Arc::new(
            database
                .fetch_graph()
                .await
                .context("Could not query network graph from database")?
                .unwrap_or_else(|| NetworkGraph::new(network, KldLogger::global())),
        );
        let scorer = Arc::new(std::sync::RwLock::new(
            database
                .fetch_scorer(
                    ProbabilisticScoringDecayParameters::default(),
                    network_graph.clone(),
                )
                .await?
                .map(|s| s.0)
                .unwrap_or_else(|| {
                    ProbabilisticScorer::new(
                        ProbabilisticScoringDecayParameters::default(),
                        network_graph.clone(),
                        KldLogger::global(),
                    )
                }),
        ));
        let random_seed_bytes: [u8; 32] = random();
        let router = Arc::new(DefaultRouter::new(
            network_graph.clone(),
            KldLogger::global(),
            random_seed_bytes,
            scorer.clone(),
            ProbabilisticScoringFeeParameters::default(),
        ));

        let mut channel_monitors = database
            .fetch_channel_monitors(keys_manager.as_ref(), keys_manager.as_ref())
            .await?;
        let mut user_config = UserConfig::default();
        user_config
            .channel_handshake_limits
            .force_announced_channel_preference = false;
        user_config.channel_handshake_config.announced_channel = true;
        user_config.channel_handshake_config.our_max_accepted_htlcs = 200;
        user_config
            .channel_handshake_config
            .max_inbound_htlc_value_in_flight_percent_of_channel = 100;
        user_config.channel_handshake_limits.max_funding_satoshis = u64::MAX;
        user_config
            .channel_handshake_limits
            .force_announced_channel_preference = false;

        let getinfo_resp = bitcoind_client.get_blockchain_info().await?;
        let chain_params = ChainParameters {
            network,
            best_block: BestBlock::new(getinfo_resp.best_block_hash, getinfo_resp.blocks as u32),
        };
        let (channel_manager_blockhash, channel_manager) = {
            if is_first_start {
                let new_channel_manager = channelmanager::ChannelManager::new(
                    fee_estimator.clone(),
                    chain_monitor.clone(),
                    broadcaster.clone(),
                    router.clone(),
                    KldLogger::global(),
                    keys_manager.clone(),
                    keys_manager.clone(),
                    keys_manager.clone(),
                    user_config,
                    chain_params,
                    0,
                );
                (getinfo_resp.best_block_hash, new_channel_manager)
            } else {
                let channel_monitor_mut_refs =
                    channel_monitors.iter_mut().map(|(_, cm)| cm).collect();
                let read_args = ChannelManagerReadArgs::new(
                    keys_manager.clone(),
                    keys_manager.clone(),
                    keys_manager.clone(),
                    fee_estimator.clone(),
                    chain_monitor.clone(),
                    broadcaster.clone(),
                    router.clone(),
                    KldLogger::global(),
                    user_config,
                    channel_monitor_mut_refs,
                );
                database
                    .fetch_channel_manager(read_args)
                    .await
                    .context("failed to query channel manager from database")?
            }
        };
        let channel_manager: Arc<ChannelManager> = Arc::new(channel_manager);

        let liquidity_manager = Arc::new(LiquidityManager::new(
            keys_manager.clone(),
            Some(LiquidityProviderConfig {}),
            channel_manager.clone(),
            None,
            chain_params,
        ));

        let gossip_sync = Arc::new_cyclic(|gossip| {
            let utxo_lookup = Arc::new(BitcoindUtxoLookup::new(
                &settings,
                bitcoind_client.clone(),
                network_graph.clone(),
                gossip.clone(),
            ));
            P2PGossipSync::new(
                network_graph.clone(),
                Some(utxo_lookup),
                KldLogger::global(),
            )
        });

        let onion_messenger: Arc<OnionMessenger> = Arc::new(OnionMessenger::new(
            keys_manager.clone(),
            keys_manager.clone(),
            KldLogger::global(),
            Arc::new(lightning::onion_message::DefaultMessageRouter {}),
            IgnoringMessageHandler {},
            IgnoringMessageHandler {},
        ));
        let ephemeral_bytes: [u8; 32] = random();
        let lightning_msg_handler = MessageHandler {
            chan_handler: channel_manager.clone(),
            route_handler: gossip_sync.clone(),
            onion_message_handler: onion_messenger,
            custom_message_handler: liquidity_manager.clone(),
        };
        let peer_manager = Arc::new(PeerManager::new(
            lightning_msg_handler,
            current_time.as_secs().try_into().unwrap(),
            &ephemeral_bytes,
            KldLogger::global(),
            keys_manager.clone(),
        ));

        let async_api_requests = Arc::new(AsyncAPIRequests::new());

        let event_handler = EventHandler::new(
            channel_manager.clone(),
            bitcoind_client.clone(),
            keys_manager.clone(),
            network_graph.clone(),
            wallet.clone(),
            database.clone(),
            peer_manager.clone(),
            async_api_requests.clone(),
            settings.clone(),
        );

        let bitcoind_client_clone = bitcoind_client.clone();
        let peer_manager_clone = peer_manager.clone();
        let wallet_clone = wallet.clone();
        let peer_port = settings.peer_port;
        let database_clone = database.clone();
        let channel_manager_clone = channel_manager.clone();
        let chain_monitor_clone = chain_monitor.clone();
        let scorer_clone = scorer.clone();
        let settings_clone = settings.clone();
        tokio::spawn(async move {
            bitcoind_client_clone
                .wait_for_blockchain_synchronisation()
                .await;
            if let Err(e) = Controller::sync_to_chain_tip(
                network,
                bitcoind_client_clone,
                chain_monitor,
                channel_manager_blockhash,
                channel_manager_clone.clone(),
                channel_monitors,
            )
            .await
            {
                error!("Fatal error {}", e.into_inner());
                std::process::exit(1)
            };

            wallet_clone.keep_sync_with_chain();
            if let Err(e) = peer_manager_clone.listen(peer_port).await {
                error!("could not listen on peer port: {e}");
                std::process::exit(1)
            };
            peer_manager_clone.keep_channel_peers_connected(
                database_clone.clone(),
                channel_manager_clone.clone(),
            );
            peer_manager_clone.broadcast_node_announcement_from_settings(settings_clone);

            tokio::spawn(async move {
                if let Err(e) = process_events_async(
                    database_clone.clone(),
                    |event| async {
                        if let Err(e) = event_handler.handle_event_async(event).await {
                            log_error(&e)
                        }
                    },
                    chain_monitor_clone,
                    channel_manager_clone,
                    GossipSync::p2p(gossip_sync),
                    peer_manager_clone,
                    KldLogger::global(),
                    Some(scorer_clone),
                    |t| {
                        let quit_signal = quit_signal.clone();
                        Box::pin(async move {
                            tokio::select! {
                                _ = tokio::time::sleep(t) => false,
                                _ = quit_signal => true,
                            }
                        })
                    },
                    false,
                )
                .await
                {
                    error!("Fatal error {}", e);
                    std::process::exit(1)
                };
            });
        });

        Ok(Controller {
            settings,
            database,
            bitcoind_client,
            channel_manager,
            peer_manager,
            keys_manager,
            network_graph,
            router,
            scorer,
            wallet,
            async_api_requests,
            _liquidity_manager: liquidity_manager,
        })
    }

    async fn sync_to_chain_tip(
        network: Network,
        bitcoind_client: Arc<BitcoindClient>,
        chain_monitor: Arc<ChainMonitor>,
        channel_manager_blockhash: BlockHash,
        channel_manager: Arc<ChannelManager>,
        channelmonitors: Vec<(BlockHash, ChannelMonitor<InMemorySigner>)>,
    ) -> BlockSourceResult<()> {
        info!(
            "Syncing ChannelManager and {} ChannelMonitors to chain tip",
            channelmonitors.len()
        );
        let mut chain_listener_channel_monitors = Vec::new();
        let mut cache = UnboundedCache::new();

        let mut chain_listeners = vec![(
            channel_manager_blockhash,
            channel_manager.as_ref() as &(dyn chain::Listen + Send + Sync),
        )];

        for (blockhash, channel_monitor) in channelmonitors {
            let outpoint = channel_monitor.get_funding_txo().0;
            chain_listener_channel_monitors.push((
                blockhash,
                (
                    channel_monitor,
                    bitcoind_client.clone(),
                    bitcoind_client.clone(),
                    KldLogger::global(),
                ),
                outpoint,
            ));
        }

        for monitor_listener_info in chain_listener_channel_monitors.iter_mut() {
            chain_listeners.push((
                monitor_listener_info.0,
                &monitor_listener_info.1 as &(dyn chain::Listen + Send + Sync),
            ));
        }
        let chain_tip = init::synchronize_listeners(
            bitcoind_client.clone(),
            network,
            &mut cache,
            chain_listeners,
        )
        .await?;
        info!("Chain listeners synchronised. Registering ChannelMonitors with ChainMonitor");
        for (_, (channel_monitor, _, _, _), funding_outpoint) in chain_listener_channel_monitors {
            if let Err(e) = chain_monitor.watch_channel(funding_outpoint, channel_monitor) {
                warn!("Could not sync info for channel: {e:?}");
            }
            info!("Registered {}", funding_outpoint.txid);
        }

        // Connect and Disconnect Blocks
        tokio::spawn(async move {
            let chain_poller = poll::ChainPoller::new(bitcoind_client, network);
            let chain_listener = (chain_monitor, channel_manager);
            let mut spv_client =
                SpvClient::new(chain_tip, chain_poller, &mut cache, &chain_listener);
            loop {
                if let Err(e) = spv_client.poll_best_tip().await {
                    error!("{}", e.into_inner())
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });

        Ok(())
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.stop()
    }
}
