use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use bitcoin::hashes::hex::FromHex;
use bitcoin::hashes::{sha256, Hash};
use bitcoin::secp256k1::{Secp256k1, SecretKey};
use bitcoin::{Network, TxOut, Txid};
use kld::database::forward::{Forward, ForwardStatus};
use kld::database::invoice::Invoice;
use kld::database::payment::{Payment, PaymentDirection};
use kld::database::peer::Peer;
use kld::database::ChannelRecord;
use kld::database::LdkDatabase;
use kld::ldk::Scorer;

use kld::logger::KldLogger;
use lightning::chain::chaininterface::{BroadcasterInterface, FeeEstimator};
use lightning::chain::chainmonitor::ChainMonitor;
use lightning::chain::transaction::OutPoint;
use lightning::chain::Filter;

use lightning::events::ClosureReason;
use lightning::ln::channelmanager::{
    ChannelCounterparty, ChannelDetails, CounterpartyForwardingInfo,
};
use lightning::ln::features::{ChannelTypeFeatures, InitFeatures};
use lightning::ln::msgs::SocketAddress;
use lightning::ln::ChannelId;
use lightning::ln::{PaymentHash, PaymentPreimage, PaymentSecret};
use lightning::routing::gossip::NetworkGraph;
use lightning::routing::router::DefaultRouter;
use lightning::routing::scoring::{
    ProbabilisticScorer, ProbabilisticScoringDecayParameters, ProbabilisticScoringFeeParameters,
};
use lightning::sign::{InMemorySigner, KeysManager, SpendableOutputDescriptor};
use lightning::util::persist::Persister;
use lightning_invoice::{Currency, InvoiceBuilder};
use rand::random;
use test_utils::{
    init_db_test_context, poll, random_public_key, TempDir, TEST_PRIVATE_KEY, TEST_PUBLIC_KEY,
    TEST_TX_ID,
};

#[tokio::test(flavor = "multi_thread")]
pub async fn test_peers() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let (settings, _cockroach, durable_connection) = init_db_test_context(&temp_dir).await?;

    let database = LdkDatabase::new(settings.into(), durable_connection.into());

    let peer = Peer {
        public_key: random_public_key(),
        address: SocketAddress::TcpIpV4 {
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
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_forwards() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let (settings, _cockroach, durable_connection) = init_db_test_context(&temp_dir).await?;

    let database = LdkDatabase::new(settings.into(), durable_connection.into());

    let amount = 1000000;
    let fee = 100;
    let forward_success = Forward::success(
        ChannelId::from_bytes([0u8; 32]),
        ChannelId::from_bytes([1u8; 32]),
        amount,
        fee,
    );
    database.persist_forward(forward_success.clone()).await?;

    let forward_fail = Forward::failure(
        ChannelId::from_bytes([3u8; 32]),
        lightning::events::HTLCDestination::FailedPayment {
            payment_hash: PaymentHash([1u8; 32]),
        },
    );
    database.persist_forward(forward_fail.clone()).await?;

    let total = database.fetch_total_forwards().await?;
    assert_eq!(1, total.count);
    assert_eq!(amount, total.amount);
    assert_eq!(fee, total.fee);

    let forwards = database.fetch_forwards(None).await?;
    assert_eq!(
        forwards.first().context("expected success forward")?,
        &forward_success
    );
    assert_eq!(
        forwards.last().context("expected failed forward")?,
        &forward_fail
    );

    let forwards = database
        .fetch_forwards(Some(ForwardStatus::Succeeded))
        .await?;
    assert_eq!(1, forwards.len());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_invoice_payments() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let (settings, _cockroach, durable_connection) = init_db_test_context(&temp_dir).await?;

    let database = LdkDatabase::new(settings.into(), durable_connection.into());

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

    let mut payment = Payment::of_invoice_outbound(&invoice, Some("label".to_string()));
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

    let stored_payments = database
        .fetch_payments(None, None)
        .await?
        .into_iter()
        .find(|p| p.id == payment.id)
        .context("expected payment")?;
    assert_eq!(stored_payments, payment);

    payment.succeeded(PaymentHash([1u8; 32]), PaymentPreimage(random()), Some(232));
    database.persist_payment(&payment).await?;

    let stored_payments = database
        .fetch_payments(payment.hash, Some(PaymentDirection::Outbound))
        .await?;
    assert_eq!(1, stored_payments.len());
    assert_eq!(
        stored_payments.first().context("expected payment")?,
        &payment
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_network_graph() -> Result<()> {
    KldLogger::init("test", log::LevelFilter::Debug);
    let temp_dir = TempDir::new()?;
    let (settings, _cockroach, durable_connection) = init_db_test_context(&temp_dir).await?;

    let database = LdkDatabase::new(settings.into(), durable_connection.into());

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
            Arc<
                DefaultRouter<
                    Arc<NetworkGraph<Arc<KldLogger>>>,
                    Arc<KldLogger>,
                    Arc<Mutex<Scorer>>,
                    ProbabilisticScoringFeeParameters,
                    Scorer,
                >,
            >,
            Arc<KldLogger>,
            Mutex<Scorer>,
        >>::persist_graph(database, network_graph)
    };
    persist(&database, &network_graph)?;
    assert!(database.fetch_graph().await.unwrap().is_some());

    network_graph.set_last_rapid_gossip_sync_timestamp(10);
    persist(&database, &network_graph)?;
    assert!(database.fetch_graph().await.unwrap().is_some());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_scorer() -> Result<()> {
    KldLogger::init("test", log::LevelFilter::Debug);
    let temp_dir = TempDir::new()?;
    let (settings, _cockroach, durable_connection) = init_db_test_context(&temp_dir).await?;

    let database = LdkDatabase::new(settings.into(), durable_connection.into());

    let network_graph = Arc::new(NetworkGraph::new(Network::Regtest, KldLogger::global()));
    let scorer = Mutex::new(ProbabilisticScorer::new(
        ProbabilisticScoringDecayParameters::default(),
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
            Arc<
                DefaultRouter<
                    Arc<NetworkGraph<Arc<KldLogger>>>,
                    Arc<KldLogger>,
                    Arc<Mutex<Scorer>>,
                    ProbabilisticScoringFeeParameters,
                    Scorer,
                >,
            >,
            Arc<KldLogger>,
            Mutex<Scorer>,
        >>::persist_scorer(database, scorer)
    };

    persist(&database, &scorer)?;
    poll!(
        3,
        database
            .fetch_scorer(
                ProbabilisticScoringDecayParameters::default(),
                network_graph.clone()
            )
            .await?
            .is_some()
    );

    let timestamp = database
        .fetch_scorer(
            ProbabilisticScoringDecayParameters::default(),
            network_graph.clone(),
        )
        .await?
        .map(|s| s.1)
        .ok_or(anyhow!("missing timestamp"))?;

    persist(&database, &scorer)?;
    poll!(
        3,
        database
            .fetch_scorer(
                ProbabilisticScoringDecayParameters::default(),
                network_graph.clone()
            )
            .await?
            .map(|s| s.1)
            .filter(|t| t > &timestamp)
            .is_some()
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_spendable_outputs() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let (settings, _cockroach, durable_connection) = init_db_test_context(&temp_dir).await?;

    let database = LdkDatabase::new(settings.into(), durable_connection.into());

    let output = TxOut::default();
    let outpoint = OutPoint {
        txid: Txid::from_hex(TEST_TX_ID)?,
        index: 2,
    };
    let descriptor = SpendableOutputDescriptor::StaticOutput { outpoint, output };
    database
        .persist_spendable_output(&descriptor, false)
        .await?;

    // Update is_spent after it spend
    database.persist_spendable_output(&descriptor, true).await?;

    let spendable_outputs = database.fetch_spendable_outputs().await?;
    assert_eq!(1, spendable_outputs.len());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_channels() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let (settings, _cockroach, durable_connection) = init_db_test_context(&temp_dir).await?;

    let database = LdkDatabase::new(settings.into(), durable_connection.into());

    let mut type_features = ChannelTypeFeatures::empty();
    type_features.set_zero_conf_optional();
    type_features.set_scid_privacy_required();
    let channel_id = ChannelId::from_bytes(random());
    let initial_channel_id = ChannelId::from_bytes(random());

    let channel = ChannelDetails {
        channel_id,
        short_channel_id: Some(u64::MAX), // i64::MAX + 1
        user_channel_id: u128::MAX,       // u64::MAX + 1
        counterparty: ChannelCounterparty {
            node_id: bitcoin::secp256k1::PublicKey::from_str(TEST_PUBLIC_KEY).unwrap(),
            features: InitFeatures::empty(),
            unspendable_punishment_reserve: 0,
            forwarding_info: Some(CounterpartyForwardingInfo {
                fee_base_msat: u32::MAX,
                fee_proportional_millionths: u32::MAX,
                cltv_expiry_delta: u16::MAX,
            }),
            outbound_htlc_minimum_msat: Some(u64::MAX),
            outbound_htlc_maximum_msat: Some(u64::MAX),
        },
        funding_txo: Some(OutPoint {
            txid: Txid::from_hex(TEST_TX_ID)?,
            index: 0,
        }),
        is_public: true,
        is_outbound: true,
        channel_value_satoshis: u64::MAX,
        channel_type: Some(ChannelTypeFeatures::empty()),
        balance_msat: u64::MAX,
        channel_shutdown_state: None,
        config: None,
        confirmations: Some(u32::MAX),
        confirmations_required: Some(u32::MAX),
        feerate_sat_per_1000_weight: Some(u32::MAX),
        force_close_spend_delay: Some(u16::MAX),
        inbound_capacity_msat: u64::MAX,
        inbound_htlc_maximum_msat: None,
        inbound_htlc_minimum_msat: None,
        inbound_scid_alias: None,
        is_channel_ready: true,
        is_usable: false,
        next_outbound_htlc_limit_msat: u64::MAX,
        next_outbound_htlc_minimum_msat: u64::MAX,
        outbound_capacity_msat: u64::MAX,
        outbound_scid_alias: Some(u64::MAX),
        unspendable_punishment_reserve: Some(u64::MAX),
    };

    database
        .persist_initial_channel(
            initial_channel_id,
            true,
            bitcoin::secp256k1::PublicKey::from_str(TEST_PUBLIC_KEY).unwrap(),
            Txid::from_hex(TEST_TX_ID)?,
        )
        .await?;
    database
        .update_initial_channel(initial_channel_id, Some((&channel_id, 0)), None::<&str>)
        .await?;

    let mut channels = database.fetch_channel_history().await?;
    assert_eq!(0, channels.len());

    database.persist_channel(&channel).await?;

    let reason = ClosureReason::CooperativeClosure;
    database
        .close_channel(&channel.channel_id, format!("{reason}"))
        .await?;

    channels = database.fetch_channel_history().await?;
    assert_eq!(1, channels.len());
    let ChannelRecord {
        open_timestamp,
        update_timestamp,
        closure_reason,
        detail,
        ..
    } = channels.first().context("expected channel")?;
    assert!(update_timestamp > open_timestamp);
    assert_eq!(*detail, Some(channel));
    assert_eq!(*closure_reason, Some(reason.to_string()));

    Ok(())
}

// XXX
// Why do we make a crate `test-utils`?
// Test utils should be collect.
type KldTestChainMonitor = ChainMonitor<
    InMemorySigner,
    Arc<dyn Filter + Send + Sync>,
    Arc<dyn BroadcasterInterface>,
    Arc<dyn FeeEstimator>,
    Arc<KldLogger>,
    Arc<LdkDatabase>,
>;
