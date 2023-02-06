use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::{Arc, Mutex};
use std::vec;

use anyhow::Result;
use bitcoin::blockdata::block::{Block, BlockHeader};
use bitcoin::hashes::Hash;
use bitcoin::{BlockHash, TxMerkleNode};
use bitcoind::Client;
use database::ldk_database::LdkDatabase;
use database::peer::Peer;

use lightning::chain::chainmonitor::ChainMonitor;
use lightning::chain::keysinterface::{InMemorySigner, KeysManager};
use lightning::chain::Filter;
use lightning::ln::{channelmanager, functional_test_utils::*};
use lightning::routing::gossip::{NetworkGraph, NodeId};
use lightning::routing::scoring::{ProbabilisticScorer, ProbabilisticScoringParameters};
use lightning::util::events::{ClosureReason, MessageSendEventsProvider};
use lightning::util::persist::Persister;
use lightning::util::test_utils as ln_utils;
use lightning::{check_added_monitors, check_closed_broadcast, check_closed_event};
use logger::KndLogger;
use test_utils::random_public_key;

use crate::{create_database, with_cockroach};

#[tokio::test(flavor = "multi_thread")]
pub async fn test_peers() -> Result<()> {
    with_cockroach(|settings| async move {
        let database = LdkDatabase::new(settings).await?;

        let peer = Peer {
            public_key: random_public_key(),
            socket_addr: std::net::SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::new(127, 0, 0, 1),
                1020,
            )),
        };
        let saved_peer = database.fetch_peer(&peer.public_key).await?;
        assert_eq!(None, saved_peer);

        database.persist_peer(&peer).await?;

        let saved_peer = database.fetch_peer(&peer.public_key).await?;
        assert_eq!(peer, saved_peer.unwrap());

        let peers = database.fetch_peers().await?;
        assert!(peers.contains(&peer));

        database.delete_peer(&peer).await?;
        let peers = database.fetch_peers().await?;
        assert!(!peers.contains(&peer));
        Ok(())
    })
    .await
}

// (Test copied from LDK FilesystemPersister).
// Test relaying a few payments and check that the persisted data is updated the appropriate number of times.
#[tokio::test(flavor = "multi_thread")]
pub async fn test_channel_monitors() -> Result<()> {
    with_cockroach(|settings| async move {
        let database_0 = LdkDatabase::new(&create_database(settings, "test1").await).await?;
        let database_1 = LdkDatabase::new(&create_database(settings, "test2").await).await?;

        // Create the nodes, giving them data databases.
        let chanmon_cfgs = create_chanmon_cfgs(2);
        let mut node_cfgs = create_node_cfgs(2, &chanmon_cfgs);
        let chain_mon_0 = ln_utils::TestChainMonitor::new(
            Some(&chanmon_cfgs[0].chain_source),
            &chanmon_cfgs[0].tx_broadcaster,
            &chanmon_cfgs[0].logger,
            &chanmon_cfgs[0].fee_estimator,
            &database_0,
            node_cfgs[0].keys_manager,
        );
        let chain_mon_1 = ln_utils::TestChainMonitor::new(
            Some(&chanmon_cfgs[1].chain_source),
            &chanmon_cfgs[1].tx_broadcaster,
            &chanmon_cfgs[1].logger,
            &chanmon_cfgs[1].fee_estimator,
            &database_1,
            node_cfgs[1].keys_manager,
        );
        node_cfgs[0].chain_monitor = chain_mon_0;
        node_cfgs[1].chain_monitor = chain_mon_1;
        let node_chanmgrs = create_node_chanmgrs(2, &node_cfgs, &[None, None]);
        let nodes = create_network(2, &node_cfgs, &node_chanmgrs);

        // Check that the persisted channel data is empty before any channels are
        // open.
        let mut persisted_chan_data_0 = database_0
            .fetch_channel_monitors(nodes[0].keys_manager)
            .await?;
        assert_eq!(persisted_chan_data_0.len(), 0);
        let mut persisted_chan_data_1 = database_1
            .fetch_channel_monitors(nodes[0].keys_manager)
            .await?;
        assert_eq!(persisted_chan_data_1.len(), 0);

        // Helper to make sure the channel is on the expected update ID.
        macro_rules! check_persisted_data {
            ($expected_update_id: expr) => {
                persisted_chan_data_0 = database_0
                    .fetch_channel_monitors(nodes[0].keys_manager)
                    .await
                    .unwrap();
                assert_eq!(persisted_chan_data_0.len(), 1);
                for (_, mon) in persisted_chan_data_0.iter() {
                    assert_eq!(mon.get_latest_update_id(), $expected_update_id);
                }
                persisted_chan_data_1 = database_1
                    .fetch_channel_monitors(nodes[0].keys_manager)
                    .await
                    .unwrap();
                assert_eq!(persisted_chan_data_1.len(), 1);
                for (_, mon) in persisted_chan_data_1.iter() {
                    assert_eq!(mon.get_latest_update_id(), $expected_update_id);
                }
            };
        }

        // Create some initial channel and check that a channel was persisted.
        let _ = create_announced_chan_between_nodes(
            &nodes,
            0,
            1,
            channelmanager::provided_init_features(),
            channelmanager::provided_init_features(),
        );
        check_persisted_data!(0);

        // Send a few payments and make sure the monitors are updated to the latest.
        send_payment(&nodes[0], &vec![&nodes[1]][..], 8000000);
        check_persisted_data!(5);
        send_payment(&nodes[1], &vec![&nodes[0]][..], 4000000);
        check_persisted_data!(10);

        // Force close because cooperative close doesn't result in any persisted
        // updates.
        nodes[0]
            .node
            .force_close_broadcasting_latest_txn(
                &nodes[0].node.list_channels()[0].channel_id,
                &nodes[1].node.get_our_node_id(),
            )
            .unwrap();
        check_closed_event!(nodes[0], 1, ClosureReason::HolderForceClosed);
        check_closed_broadcast!(nodes[0], true);
        check_added_monitors!(nodes[0], 1);

        let node_txn = nodes[0].tx_broadcaster.txn_broadcasted.lock().unwrap();
        assert_eq!(node_txn.len(), 1);

        let header = BlockHeader {
            version: 0x20000000,
            prev_blockhash: nodes[0].best_block_hash(),
            merkle_root: TxMerkleNode::all_zeros(),
            time: 42,
            bits: 42,
            nonce: 42,
        };
        connect_block(
            &nodes[1],
            &Block {
                header,
                txdata: vec![node_txn[0].clone(), node_txn[0].clone()],
            },
        );
        check_closed_broadcast!(nodes[1], true);
        check_closed_event!(nodes[1], 1, ClosureReason::CommitmentTxConfirmed);
        check_added_monitors!(nodes[1], 1);

        // Make sure everything is persisted as expected after close.
        check_persisted_data!(11);
        Ok(())
    })
    .await
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_network_graph() -> Result<()> {
    with_cockroach(|settings| async move {
        let database = LdkDatabase::new(settings).await?;

        let network_graph = Arc::new(NetworkGraph::new(
            BlockHash::all_zeros(),
            KndLogger::global(),
        ));
        // how to make this less verbose?
        let persist = |database, network_graph| {
            <LdkDatabase as Persister<
                '_,
                Arc<KndTestChainMonitor>,
                Arc<Client>,
                Arc<KeysManager>,
                Arc<Client>,
                Arc<KndLogger>,
                TestScorer,
            >>::persist_graph(database, network_graph)
        };

        persist(&database, &network_graph)?;
        assert!(database.fetch_graph().await.unwrap().is_some());

        network_graph.set_last_rapid_gossip_sync_timestamp(10);
        persist(&database, &network_graph)?;
        assert!(database.fetch_graph().await.unwrap().is_some());

        Ok(())
    })
    .await
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_scorer() -> Result<()> {
    with_cockroach(|settings| async move {
        let database = LdkDatabase::new(settings).await?;

        let network_graph = Arc::new(NetworkGraph::new(
            BlockHash::all_zeros(),
            KndLogger::global(),
        ));
        let scorer = Mutex::new(ProbabilisticScorer::new(
            ProbabilisticScoringParameters::default(),
            network_graph.clone(),
            KndLogger::global(),
        ));
        let persist = |database, scorer| {
            <LdkDatabase as Persister<
                '_,
                Arc<KndTestChainMonitor>,
                Arc<Client>,
                Arc<KeysManager>,
                Arc<Client>,
                Arc<KndLogger>,
                TestScorer,
            >>::persist_scorer(database, scorer)
        };

        persist(&database, &scorer)?;
        assert!(database
            .fetch_scorer(
                ProbabilisticScoringParameters::default(),
                network_graph.clone()
            )
            .await?
            .is_some());

        scorer
            .lock()
            .unwrap()
            .add_banned(&NodeId::from_pubkey(&random_public_key()));

        persist(&database, &scorer)?;
        assert!(database
            .fetch_scorer(
                ProbabilisticScoringParameters::default(),
                network_graph.clone()
            )
            .await?
            .is_some());
        Ok(())
    })
    .await
}

type TestScorer = Mutex<ProbabilisticScorer<Arc<NetworkGraph<Arc<KndLogger>>>, Arc<KndLogger>>>;

type KndTestChainMonitor = ChainMonitor<
    InMemorySigner,
    Arc<dyn Filter + Send + Sync>,
    Arc<Client>,
    Arc<Client>,
    Arc<KndLogger>,
    Arc<LdkDatabase>,
>;
