use crate::bitcoind_client::BitcoindClient;
use crate::event_handler::EventHandler;
use crate::logger::LightningLogger;
use crate::net_utils::do_connect_peer;
use crate::payment_info::PaymentInfoStorage;
use crate::settings::Settings;
use crate::{disk, net_utils};
use anyhow::{bail, Result};
use bitcoin::blockdata::constants::genesis_block;
use bitcoin::BlockHash;
use lightning::chain;
use lightning::chain::chainmonitor;
use lightning::chain::keysinterface::{InMemorySigner, KeysInterface, KeysManager, Recipient};
use lightning::chain::{BestBlock, Filter, Watch};
use lightning::ln::channelmanager;
use lightning::ln::channelmanager::{
    ChainParameters, ChannelManagerReadArgs, SimpleArcChannelManager,
};
use lightning::ln::peer_handler::{IgnoringMessageHandler, MessageHandler, SimpleArcPeerManager};
use lightning::onion_message::SimpleArcOnionMessenger;
use lightning::routing::gossip;
use lightning::routing::gossip::P2PGossipSync;
use lightning::routing::scoring::ProbabilisticScorer;
use lightning::util::config::UserConfig;
use lightning::util::ser::ReadableArgs;
use lightning_background_processor::{BackgroundProcessor, GossipSync};
use lightning_block_sync::init;
use lightning_block_sync::poll;
use lightning_block_sync::SpvClient;
use lightning_block_sync::UnboundedCache;
use lightning_invoice::payment;
use lightning_invoice::utils::DefaultRouter;
use lightning_net_tokio::SocketDescriptor;
use lightning_persister::FilesystemPersister;
use rand::{thread_rng, Rng};
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::ops::Deref;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

pub(crate) struct Controller {
    peer_manager: Arc<PeerManager>,
    network_graph: Arc<NetworkGraph>,
}

impl Controller {
    pub fn num_nodes(&self) -> usize {
        self.network_graph.read_only().nodes().len()
    }

    pub fn num_channels(&self) -> usize {
        self.network_graph.read_only().channels().len()
    }

    pub fn num_peers(&self) -> usize {
        self.peer_manager.get_peer_node_ids().len()
    }

    pub fn stop(&self) {
        // Disconnect our peers and stop accepting new connections. This ensures we don't continue
        // updating our channel data after we've stopped the background processor.
        self.peer_manager.disconnect_all_peers();
    }

    pub async fn start_ldk(
        settings: &Settings,
        shutdown_flag: Arc<AtomicBool>,
    ) -> Result<(Controller, BackgroundProcessor)> {
        // Initialize the LDK data directory if necessary.
        let ldk_data_dir = format!("{}/data", settings.knd_storage_dir);
        fs::create_dir_all(ldk_data_dir.clone()).unwrap();

        // Initialize our bitcoind client.
        let bitcoind_client =
            match BitcoindClient::new(settings, tokio::runtime::Handle::current()).await {
                Ok(client) => Arc::new(client),
                Err(e) => {
                    bail!("Failed to connect to bitcoind client: {}", e);
                }
            };

        // Check that the bitcoind we've connected to is running the network we expect
        let bitcoind_chain = bitcoind_client.get_blockchain_info().await.chain;
        if bitcoind_chain
            != match settings.bitcoin_network {
                bitcoin::Network::Bitcoin => "main",
                bitcoin::Network::Testnet => "test",
                bitcoin::Network::Regtest => "regtest",
                bitcoin::Network::Signet => "signet",
            }
        {
            bail!(
                "Chain argument ({}) didn't match bitcoind chain ({})",
                settings.bitcoin_network,
                bitcoind_chain
            );
        }

        // ## Setup
        // Step 1: Initialize the FeeEstimator

        // BitcoindClient implements the FeeEstimator trait, so it'll act as our fee estimator.
        let fee_estimator = bitcoind_client.clone();

        // Step 2: Initialize the Logger
        let logger = Arc::new(LightningLogger::default());

        // Step 3: Initialize the BroadcasterInterface

        // BitcoindClient implements the BroadcasterInterface trait, so it'll act as our transaction
        // broadcaster.
        let broadcaster = bitcoind_client.clone();

        // Step 4: Initialize Persist
        let persister = Arc::new(FilesystemPersister::new(ldk_data_dir.clone()));

        // Step 5: Initialize the ChainMonitor
        let chain_monitor: Arc<ChainMonitor> = Arc::new(chainmonitor::ChainMonitor::new(
            None,
            broadcaster.clone(),
            logger.clone(),
            fee_estimator.clone(),
            persister.clone(),
        ));

        // Step 6: Initialize the KeysManager

        // The key seed that we use to derive the node privkey (that corresponds to the node pubkey) and
        // other secret key material.
        let keys_seed_path = format!("{}/keys_seed", ldk_data_dir.clone());
        let keys_seed = if let Ok(seed) = fs::read(keys_seed_path.clone()) {
            assert_eq!(seed.len(), 32);
            let mut key = [0; 32];
            key.copy_from_slice(&seed);
            key
        } else {
            let key: [u8; 32] = thread_rng().gen();
            match File::create(keys_seed_path.clone()) {
                Ok(mut f) => {
                    f.write_all(&key)
                        .expect("Failed to write node keys seed to disk");
                    f.sync_all().expect("Failed to sync node keys seed to disk");
                }
                Err(e) => {
                    bail!(
                        "ERROR: Unable to create keys seed file {}: {}",
                        keys_seed_path,
                        e
                    );
                }
            }
            key
        };
        let cur = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let keys_manager = Arc::new(KeysManager::new(
            &keys_seed,
            cur.as_secs(),
            cur.subsec_nanos(),
        ));

        // Step 7: Read ChannelMonitor state from disk
        let mut channelmonitors = persister
            .read_channelmonitors(keys_manager.clone())
            .unwrap();

        // Step 8: Initialize the ChannelManager
        let mut user_config = UserConfig::default();
        user_config
            .channel_handshake_limits
            .force_announced_channel_preference = false;
        let mut restarting_node = true;
        let (channel_manager_blockhash, channel_manager) = {
            if let Ok(mut f) = fs::File::open(format!("{}/manager", ldk_data_dir.clone())) {
                let mut channel_monitor_mut_references = Vec::new();
                for (_, channel_monitor) in channelmonitors.iter_mut() {
                    channel_monitor_mut_references.push(channel_monitor);
                }
                let read_args = ChannelManagerReadArgs::new(
                    keys_manager.clone(),
                    fee_estimator.clone(),
                    chain_monitor.clone(),
                    broadcaster.clone(),
                    logger.clone(),
                    user_config,
                    channel_monitor_mut_references,
                );
                <(BlockHash, ChannelManager)>::read(&mut f, read_args).unwrap()
            } else {
                // We're starting a fresh node.
                restarting_node = false;
                let getinfo_resp = bitcoind_client.get_blockchain_info().await;

                let chain_params = ChainParameters {
                    network: settings.bitcoin_network,
                    best_block: BestBlock::new(
                        getinfo_resp.latest_blockhash,
                        getinfo_resp.latest_height as u32,
                    ),
                };
                let fresh_channel_manager = channelmanager::ChannelManager::new(
                    fee_estimator.clone(),
                    chain_monitor.clone(),
                    broadcaster.clone(),
                    logger.clone(),
                    keys_manager.clone(),
                    user_config,
                    chain_params,
                );
                (getinfo_resp.latest_blockhash, fresh_channel_manager)
            }
        };

        // Step 9: Sync ChannelMonitors and ChannelManager to chain tip
        let mut chain_listener_channel_monitors = Vec::new();
        let mut cache = UnboundedCache::new();
        let mut chain_tip: Option<poll::ValidatedBlockHeader> = None;
        if restarting_node {
            let mut chain_listeners = vec![(
                channel_manager_blockhash,
                &channel_manager as &(dyn chain::Listen + Send + Sync),
            )];

            for (blockhash, channel_monitor) in channelmonitors.drain(..) {
                let outpoint = channel_monitor.get_funding_txo().0;
                chain_listener_channel_monitors.push((
                    blockhash,
                    (
                        channel_monitor,
                        broadcaster.clone(),
                        fee_estimator.clone(),
                        logger.clone(),
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
            chain_tip = Some(
                init::synchronize_listeners(
                    &mut bitcoind_client.deref(),
                    settings.bitcoin_network,
                    &mut cache,
                    chain_listeners,
                )
                .await
                .unwrap(),
            );
        }

        // Step 10: Give ChannelMonitors to ChainMonitor
        for item in chain_listener_channel_monitors.drain(..) {
            let channel_monitor = item.1 .0;
            let funding_outpoint = item.2;
            chain_monitor
                .watch_channel(funding_outpoint, channel_monitor)
                .unwrap();
        }

        // Step 11: Optional: Initialize the P2PGossipSync
        let genesis = genesis_block(settings.bitcoin_network).header.block_hash();
        let network_graph_path = format!("{}/network_graph", ldk_data_dir.clone());
        let network_graph = Arc::new(disk::read_network(
            Path::new(&network_graph_path),
            genesis,
            logger.clone(),
        ));
        let gossip_sync = Arc::new(P2PGossipSync::new(
            network_graph.clone(),
            None::<Arc<dyn chain::Access + Send + Sync>>,
            logger.clone(),
        ));

        // Step 12: Initialize the PeerManager
        let channel_manager: Arc<ChannelManager> = Arc::new(channel_manager);
        let onion_messenger: Arc<OnionMessenger> =
            Arc::new(OnionMessenger::new(keys_manager.clone(), logger.clone()));
        let ephemeral_bytes: [u8; 32] = thread_rng().gen();
        let current_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let lightning_msg_handler = MessageHandler {
            chan_handler: channel_manager.clone(),
            route_handler: gossip_sync.clone(),
            onion_message_handler: onion_messenger.clone(),
        };
        let peer_manager: Arc<PeerManager> = Arc::new(PeerManager::new(
            lightning_msg_handler,
            keys_manager.get_node_secret(Recipient::Node).unwrap(),
            current_time,
            &ephemeral_bytes,
            logger.clone(),
            IgnoringMessageHandler {},
        ));

        // ## Running LDK
        // Step 13: Initialize networking

        let peer_manager_connection_handler = peer_manager.clone();
        let listening_port = settings.knd_peer_port;
        let stop_listen = shutdown_flag.clone();
        tokio::spawn(async move {
            let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", listening_port))
                .await
                .expect(
                    "Failed to bind to listen port - is something else already listening on it?",
                );
            loop {
                let peer_mgr = peer_manager_connection_handler.clone();
                let tcp_stream = listener.accept().await.unwrap().0;
                if stop_listen.load(Ordering::Acquire) {
                    return;
                }
                tokio::spawn(async move {
                    lightning_net_tokio::setup_inbound(
                        peer_mgr.clone(),
                        tcp_stream.into_std().unwrap(),
                    )
                    .await;
                });
            }
        });

        // Step 14: Connect and Disconnect Blocks
        if chain_tip.is_none() {
            chain_tip = Some(
                init::validate_best_block_header(&mut bitcoind_client.deref())
                    .await
                    .unwrap(),
            );
        }
        let channel_manager_listener = channel_manager.clone();
        let chain_monitor_listener = chain_monitor.clone();
        let bitcoind_block_source = bitcoind_client.clone();
        let network = settings.bitcoin_network;
        tokio::spawn(async move {
            let mut derefed = bitcoind_block_source.deref();
            let chain_poller = poll::ChainPoller::new(&mut derefed, network);
            let chain_listener = (chain_monitor_listener, channel_manager_listener);
            let mut spv_client = SpvClient::new(
                chain_tip.unwrap(),
                chain_poller,
                &mut cache,
                &chain_listener,
            );
            loop {
                spv_client.poll_best_tip().await.unwrap();
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });

        // Step 15: Handle LDK Events
        // TODO: persist payment info to disk
        let inbound_payments: PaymentInfoStorage = Arc::new(Mutex::new(HashMap::new()));
        let outbound_payments: PaymentInfoStorage = Arc::new(Mutex::new(HashMap::new()));
        let event_handler = EventHandler::new(
            channel_manager.clone(),
            bitcoind_client,
            keys_manager.clone(),
            inbound_payments,
            outbound_payments,
            settings.bitcoin_network,
            network_graph.clone(),
        );

        // Step 16: Initialize routing ProbabilisticScorer
        let scorer_path = format!("{}/scorer", ldk_data_dir.clone());
        let scorer = Arc::new(Mutex::new(disk::read_scorer(
            Path::new(&scorer_path),
            Arc::clone(&network_graph),
            Arc::clone(&logger),
        )));

        // Step 17: Create InvoicePayer
        let router = DefaultRouter::new(
            network_graph.clone(),
            logger.clone(),
            keys_manager.get_secure_random_bytes(),
        );
        let invoice_payer = Arc::new(InvoicePayer::new(
            channel_manager.clone(),
            router,
            scorer.clone(),
            logger.clone(),
            event_handler,
            payment::Retry::Timeout(Duration::from_secs(10)),
        ));

        // Step 18: Persist ChannelManager and NetworkGraph
        let persister = Arc::new(FilesystemPersister::new(ldk_data_dir.clone()));

        // Step 19: Background Processing
        let background_processor = BackgroundProcessor::start(
            persister,
            invoice_payer.clone(),
            chain_monitor.clone(),
            channel_manager.clone(),
            GossipSync::p2p(gossip_sync.clone()),
            peer_manager.clone(),
            logger.clone(),
            Some(scorer),
        );

        // Regularly reconnect to channel peers.
        let connect_cm = Arc::clone(&channel_manager);
        let connect_pm = Arc::clone(&peer_manager);
        let peer_data_path = format!("{}/channel_peer_data", ldk_data_dir.clone());
        let stop_connect = shutdown_flag.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                match disk::read_channel_peer_data(Path::new(&peer_data_path)) {
                    Ok(info) => {
                        let peers = connect_pm.get_peer_node_ids();
                        for node_id in connect_cm
                            .list_channels()
                            .iter()
                            .map(|chan| chan.counterparty.node_id)
                            .filter(|id| !peers.contains(id))
                        {
                            if stop_connect.load(Ordering::Acquire) {
                                return;
                            }
                            for (pubkey, peer_addr) in info.iter() {
                                if *pubkey == node_id {
                                    let _ = do_connect_peer(
                                        *pubkey,
                                        *peer_addr,
                                        Arc::clone(&connect_pm),
                                    )
                                    .await;
                                }
                            }
                        }
                    }
                    Err(e) => println!(
                        "ERROR: errored reading channel peer info from disk: {:?}",
                        e
                    ),
                }
            }
        });

        // Regularly broadcast our node_announcement. This is only required (or possible) if we have
        // some public channels, and is only useful if we have public listen address(es) to announce.
        // In a production environment, this should occur only after the announcement of new channels
        // to avoid churn in the global network graph.
        if settings.knd_node_name.len() > 32 {
            bail!("Node Alias can not be longer than 32 bytes");
        }
        let mut alias = [0; 32];
        alias[..settings.knd_node_name.len()].copy_from_slice(settings.knd_node_name.as_bytes());
        let peer_man = Arc::clone(&peer_manager);
        if !settings.knd_listen_addresses.is_empty() {
            let addresses = settings.knd_listen_addresses.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(60));
                loop {
                    interval.tick().await;
                    peer_man.broadcast_node_announcement(
                        [0; 3],
                        alias,
                        addresses.iter().map(|s| net_utils::to_address(s)).collect(),
                    );
                }
            });
        }

        Ok((
            Controller {
                peer_manager,
                network_graph,
            },
            background_processor,
        ))
    }
}

type ChainMonitor = chainmonitor::ChainMonitor<
    InMemorySigner,
    Arc<dyn Filter + Send + Sync>,
    Arc<BitcoindClient>,
    Arc<BitcoindClient>,
    Arc<LightningLogger>,
    Arc<FilesystemPersister>,
>;

pub(crate) type PeerManager = SimpleArcPeerManager<
    SocketDescriptor,
    ChainMonitor,
    BitcoindClient,
    BitcoindClient,
    dyn chain::Access + Send + Sync,
    LightningLogger,
>;

pub(crate) type ChannelManager =
    SimpleArcChannelManager<ChainMonitor, BitcoindClient, BitcoindClient, LightningLogger>;

pub(crate) type InvoicePayer<E> = payment::InvoicePayer<
    Arc<ChannelManager>,
    Router,
    Arc<Mutex<ProbabilisticScorer<Arc<NetworkGraph>, Arc<LightningLogger>>>>,
    Arc<LightningLogger>,
    E,
>;

type Router = DefaultRouter<Arc<NetworkGraph>, Arc<LightningLogger>>;

pub(crate) type NetworkGraph = gossip::NetworkGraph<Arc<LightningLogger>>;

type OnionMessenger = SimpleArcOnionMessenger<LightningLogger>;
