use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Context, Result};
use bitcoin::hashes::{sha256, Hash};
use bitcoin::secp256k1::{Secp256k1, SecretKey};
use bitcoin::Network;
use kld::database::invoice::Invoice;
use kld::database::payment::{MillisatAmount, Payment, PaymentDirection, PaymentStatus};
use kld::database::peer::Peer;
use kld::database::LdkDatabase;

use kld::logger::KldLogger;
use lightning::chain::chaininterface::{BroadcasterInterface, FeeEstimator};
use lightning::chain::chainmonitor::ChainMonitor;
use lightning::chain::keysinterface::{InMemorySigner, KeysManager};
use lightning::chain::Filter;
use lightning::ln::channelmanager::PaymentId;
use lightning::ln::msgs::NetAddress;
use lightning::ln::{PaymentHash, PaymentPreimage, PaymentSecret};
use lightning::routing::gossip::{NetworkGraph, NodeId};
use lightning::routing::router::DefaultRouter;
use lightning::routing::scoring::{ProbabilisticScorer, ProbabilisticScoringParameters};
use lightning::util::persist::Persister;
use lightning_invoice::{Currency, InvoiceBuilder};
use rand::random;
use test_utils::{poll, random_public_key, TEST_PRIVATE_KEY};

use super::with_cockroach;

#[tokio::test(flavor = "multi_thread")]
pub async fn test_peers() -> Result<()> {
    with_cockroach(|settings, durable_connection| async move {
        let database = LdkDatabase::new(settings, durable_connection);

        let peer = Peer {
            public_key: random_public_key(),
            net_address: NetAddress::IPv4 {
                addr: [128, 23, 34, 2],
                port: 1000,
            },
        };
        let saved_peer = database.fetch_peer(&peer.public_key).await?;
        assert_eq!(None, saved_peer);

        database.persist_peer(&peer).await?;

        let saved_peer = database.fetch_peer(&peer.public_key).await?;
        assert_eq!(peer, saved_peer.unwrap());

        let peers = database.fetch_peers().await?;
        assert!(peers.contains_key(&peer.public_key));

        database.delete_peer(&peer.public_key).await?;
        let peers = database.fetch_peers().await?;
        assert!(!peers.contains_key(&peer.public_key));
        Ok(())
    })
    .await
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_invoice() -> Result<()> {
    with_cockroach(|settings, durable_connection| async move {
        let database = LdkDatabase::new(settings, durable_connection);

        let private_key = SecretKey::from_slice(&TEST_PRIVATE_KEY)?;
        let payment_hash = sha256::Hash::from_slice(&[1u8; 32]).unwrap();
        let payment_secret = PaymentSecret([2u8; 32]);

        let bolt11 = InvoiceBuilder::new(Currency::Regtest)
            .description("test".into())
            .amount_milli_satoshis(1000)
            .payment_hash(payment_hash)
            .payment_secret(payment_secret)
            .current_timestamp()
            .expiry_time(Duration::from_secs(3600))
            .min_final_cltv_expiry_delta(144)
            .build_signed(|hash| Secp256k1::new().sign_ecdsa_recoverable(hash, &private_key))?;

        let label = "test label".to_owned();
        let invoice = Invoice::new(Some(label.clone()), bolt11)?;
        database.persist_invoice(&invoice).await?;

        let result = database
            .fetch_invoices(Some(label.clone()))
            .await?
            .into_iter()
            .last()
            .context("expected invoice")?;
        assert_eq!(result, invoice);

        let payment = Payment {
            id: PaymentId(random()),
            hash: invoice.payment_hash,
            preimage: None,
            secret: Some(PaymentSecret(random())),
            label: Some("label".to_string()),
            status: PaymentStatus::Succeeded,
            amount: MillisatAmount(1000),
            fee: None,
            direction: PaymentDirection::Inbound,
            timestamp: SystemTime::UNIX_EPOCH,
        };
        database.persist_payment(&payment).await?;

        let result = database
            .fetch_invoices(Some(label.clone()))
            .await?
            .into_iter()
            .last()
            .context("expected invoice")?;
        assert_eq!(1, result.payments.len());

        let result = database.fetch_invoices(None).await?;
        assert_eq!(1, result.len());

        Ok(())
    })
    .await
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_payment() -> Result<()> {
    with_cockroach(|settings, durable_connection| async move {
        let database = LdkDatabase::new(settings, durable_connection);

        let mut payment = Payment {
            id: PaymentId(random()),
            hash: PaymentHash(random()),
            preimage: None,
            secret: Some(PaymentSecret(random())),
            label: Some("label".to_string()),
            status: PaymentStatus::Pending,
            amount: MillisatAmount(500000),
            fee: None,
            direction: PaymentDirection::Outbound,
            timestamp: SystemTime::UNIX_EPOCH,
        };
        database.persist_payment(&payment).await?;

        let result = database
            .fetch_payments()
            .await?
            .into_iter()
            .find(|p| p.id == payment.id)
            .context("expected payment")?;
        assert_eq!(result, payment);

        payment.preimage = Some(PaymentPreimage(random()));
        payment.status = PaymentStatus::Succeeded;
        payment.fee = Some(MillisatAmount(232));
        database.persist_payment(&payment).await?;

        let result = database
            .fetch_payments()
            .await?
            .into_iter()
            .find(|p| p.id == payment.id)
            .context("expected payment")?;
        assert_eq!(result, payment);

        Ok(())
    })
    .await
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_network_graph() -> Result<()> {
    with_cockroach(|settings, durable_connection| async move {
        let database = LdkDatabase::new(settings, durable_connection);

        let network_graph = Arc::new(NetworkGraph::new(Network::Regtest, KldLogger::global()));
        // how to make this less verbose?
        let persist = |database, network_graph| {
            <LdkDatabase as Persister<
                '_,
                Arc<KldTestChainMonitor>,
                Arc<dyn BroadcasterInterface>,
                Arc<KeysManager>,
                Arc<KeysManager>,
                Arc<KeysManager>,
                Arc<dyn FeeEstimator>,
                Arc<DefaultRouter<Arc<NetworkGraph<Arc<KldLogger>>>, Arc<KldLogger>, &TestScorer>>,
                Arc<KldLogger>,
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
    with_cockroach(|settings, durable_connection| async move {
        let database = LdkDatabase::new(settings, durable_connection);

        let network_graph = Arc::new(NetworkGraph::new(Network::Regtest, KldLogger::global()));
        let scorer = Mutex::new(ProbabilisticScorer::new(
            ProbabilisticScoringParameters::default(),
            network_graph.clone(),
            KldLogger::global(),
        ));
        let persist = |database, scorer| {
            <LdkDatabase as Persister<
                '_,
                Arc<KldTestChainMonitor>,
                Arc<dyn BroadcasterInterface>,
                Arc<KeysManager>,
                Arc<KeysManager>,
                Arc<KeysManager>,
                Arc<dyn FeeEstimator>,
                Arc<DefaultRouter<Arc<NetworkGraph<Arc<KldLogger>>>, Arc<KldLogger>, &TestScorer>>,
                Arc<KldLogger>,
                TestScorer,
            >>::persist_scorer(database, scorer)
        };

        persist(&database, &scorer)?;
        poll!(
            3,
            database
                .fetch_scorer(
                    ProbabilisticScoringParameters::default(),
                    network_graph.clone()
                )
                .await?
                .is_some()
        );

        let timestamp = database
            .fetch_scorer(
                ProbabilisticScoringParameters::default(),
                network_graph.clone(),
            )
            .await?
            .map(|s| s.1)
            .ok_or(anyhow!("missing timestamp"))?;

        scorer
            .lock()
            .unwrap()
            .add_banned(&NodeId::from_pubkey(&random_public_key()));

        persist(&database, &scorer)?;
        poll!(
            3,
            database
                .fetch_scorer(
                    ProbabilisticScoringParameters::default(),
                    network_graph.clone()
                )
                .await?
                .map(|s| s.1)
                .filter(|t| t > &timestamp)
                .is_some()
        );
        Ok(())
    })
    .await
}

type TestScorer = Mutex<ProbabilisticScorer<Arc<NetworkGraph<Arc<KldLogger>>>, Arc<KldLogger>>>;

type KldTestChainMonitor = ChainMonitor<
    InMemorySigner,
    Arc<dyn Filter + Send + Sync>,
    Arc<dyn BroadcasterInterface>,
    Arc<dyn FeeEstimator>,
    Arc<KldLogger>,
    Arc<LdkDatabase>,
>;
