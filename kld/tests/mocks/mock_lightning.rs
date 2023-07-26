use std::{
    net::{SocketAddrV4, SocketAddrV6},
    str::FromStr,
    time::{Duration, UNIX_EPOCH},
};

use anyhow::Result;
use api::FeeRate;
use async_trait::async_trait;
use bitcoin::{
    consensus::deserialize,
    hashes::{sha256, Hash},
    secp256k1::{PublicKey, Secp256k1, SecretKey},
    Network, Txid,
};
use hex::FromHex;
use kld::api::NetAddress;
use kld::{
    database::{
        invoice::Invoice,
        payment::{Payment, PaymentDirection, PaymentStatus},
    },
    ldk::{LightningInterface, OpenChannelResult, Peer, PeerStatus},
    MillisatAmount,
};
use lightning::{
    chain::transaction::OutPoint,
    ln::{
        channelmanager::{ChannelCounterparty, ChannelDetails, PaymentId},
        features::{Features, InitFeatures},
        PaymentHash, PaymentPreimage, PaymentSecret,
    },
    routing::gossip::{ChannelInfo, NodeAlias, NodeAnnouncementInfo, NodeId, NodeInfo},
    util::{config::UserConfig, indexed_map::IndexedMap},
};

use lightning_invoice::{Currency, InvoiceBuilder};
use rand::random;
use test_utils::{
    TEST_ALIAS, TEST_PRIVATE_KEY, TEST_PUBLIC_KEY, TEST_SHORT_CHANNEL_ID, TEST_TX, TEST_TX_ID,
};

pub struct MockLightning {
    pub num_peers: usize,
    pub num_nodes: usize,
    pub num_channels: usize,
    pub wallet_balance: u64,
    pub channels: Vec<ChannelDetails>,
    pub public_key: PublicKey,
    pub ipv4_address: NetAddress,
    pub invoice: Invoice,
    pub payment: Payment,
}

impl Default for MockLightning {
    fn default() -> Self {
        let public_key = PublicKey::from_str(TEST_PUBLIC_KEY).unwrap();
        let channel = ChannelDetails {
            channel_id: [1u8; 32],
            counterparty: ChannelCounterparty {
                node_id: public_key,
                features: InitFeatures::empty(),
                unspendable_punishment_reserve: 5000,
                forwarding_info: None,
                outbound_htlc_minimum_msat: Some(1000),
                outbound_htlc_maximum_msat: Some(100),
            },
            funding_txo: Some(OutPoint {
                txid: Txid::from_str(TEST_TX_ID).unwrap(),
                index: 2,
            }),
            channel_type: None,
            short_channel_id: Some(TEST_SHORT_CHANNEL_ID),
            outbound_scid_alias: None,
            inbound_scid_alias: None,
            channel_value_satoshis: 1000000,
            unspendable_punishment_reserve: Some(10000),
            user_channel_id: 3434232,
            balance_msat: 100000,
            outbound_capacity_msat: 100000,
            next_outbound_htlc_minimum_msat: 1,
            next_outbound_htlc_limit_msat: 500,
            inbound_capacity_msat: 999900000,
            confirmations_required: Some(3),
            confirmations: Some(10),
            force_close_spend_delay: Some(6),
            is_outbound: true,
            is_channel_ready: true,
            is_usable: true,
            is_public: true,
            inbound_htlc_minimum_msat: Some(300),
            inbound_htlc_maximum_msat: Some(300000),
            config: None,
            feerate_sat_per_1000_weight: Some(10210),
            channel_shutdown_state: None,
        };
        let socket_addr: SocketAddrV4 = "127.0.0.1:5555".parse().unwrap();
        let private_key = SecretKey::from_slice(&TEST_PRIVATE_KEY).unwrap();
        let public_key = PublicKey::from_str(TEST_PUBLIC_KEY).unwrap();
        let payment_hash = sha256::Hash::from_slice(&[1u8; 32]).unwrap();
        let payment_secret = PaymentSecret([2u8; 32]);
        let invoice = InvoiceBuilder::new(Currency::Regtest)
            .description("test invoice description".to_owned())
            .payee_pub_key(public_key)
            .payment_hash(payment_hash)
            .payment_secret(payment_secret)
            .min_final_cltv_expiry_delta(144)
            .expiry_time(Duration::from_secs(2322))
            .amount_milli_satoshis(200000)
            .current_timestamp()
            .build_signed(|hash| Secp256k1::new().sign_ecdsa_recoverable(hash, &private_key))
            .unwrap();
        let invoice =
            kld::database::invoice::Invoice::new(Some("label".to_string()), invoice).unwrap();
        let payment = Payment {
            id: PaymentId(random()),
            hash: PaymentHash(random()),
            preimage: Some(PaymentPreimage(random())),
            secret: Some(PaymentSecret(random())),
            label: Some("label".to_string()),
            status: PaymentStatus::Succeeded,
            amount: 100000,
            fee: Some(2323),
            direction: PaymentDirection::Outbound,
            timestamp: UNIX_EPOCH,
            bolt11: Some(invoice.bolt11.to_string()),
        };
        Self {
            num_peers: 5,
            num_nodes: 6,
            num_channels: 7,
            wallet_balance: 8,
            channels: vec![channel],
            public_key,
            ipv4_address: socket_addr.into(),
            invoice,
            payment,
        }
    }
}

#[async_trait]
impl LightningInterface for MockLightning {
    fn alias(&self) -> String {
        "test".to_string()
    }
    fn identity_pubkey(&self) -> PublicKey {
        self.public_key
    }
    async fn synced(&self) -> Result<bool> {
        Ok(true)
    }

    fn sign(&self, _message: &[u8]) -> Result<String> {
        Ok("1234abcd".to_string())
    }

    fn graph_num_nodes(&self) -> usize {
        self.num_nodes
    }

    fn graph_num_channels(&self) -> usize {
        self.num_channels
    }

    fn network(&self) -> bitcoin::Network {
        Network::Bitcoin
    }
    fn num_active_channels(&self) -> usize {
        0
    }

    fn num_inactive_channels(&self) -> usize {
        0
    }

    fn num_pending_channels(&self) -> usize {
        0
    }
    fn num_peers(&self) -> usize {
        self.num_peers
    }

    fn wallet_balance(&self) -> u64 {
        self.wallet_balance
    }

    fn list_channels(&self) -> Vec<ChannelDetails> {
        self.channels.clone()
    }

    fn set_channel_fee(
        &self,
        _counterparty_node_id: &PublicKey,
        _channel_id: &[[u8; 32]],
        forwarding_fee_proportional_millionths: Option<u32>,
        forwarding_fee_base_msat: Option<u32>,
    ) -> Result<(u32, u32)> {
        Ok((
            forwarding_fee_base_msat.unwrap_or(5000),
            forwarding_fee_proportional_millionths.unwrap_or(200),
        ))
    }

    fn alias_of(&self, _node_id: &PublicKey) -> Option<String> {
        Some(TEST_ALIAS.to_string())
    }

    fn public_addresses(&self) -> Vec<NetAddress> {
        let addr1: SocketAddrV4 = "127.0.0.1:2312".parse().unwrap();
        let addr2: SocketAddrV6 = "[2001:db8::1]:8080".parse().unwrap();
        vec![addr1.into(), addr2.into()]
    }

    async fn open_channel(
        &self,
        _their_network_key: PublicKey,
        _channel_value_satoshis: u64,
        _push_msat: Option<u64>,
        _fee_rate: Option<FeeRate>,
        _override_config: Option<UserConfig>,
    ) -> Result<OpenChannelResult> {
        let transaction =
            deserialize::<bitcoin::Transaction>(&Vec::<u8>::from_hex(TEST_TX).unwrap()).unwrap();
        let txid = transaction.txid();
        Ok(OpenChannelResult {
            transaction,
            txid,
            channel_id: [1u8; 32],
        })
    }

    async fn list_peers(&self) -> Result<Vec<Peer>> {
        Ok(vec![Peer {
            public_key: self.public_key,
            net_address: Some(self.ipv4_address.clone()),
            status: PeerStatus::Connected,
            alias: TEST_ALIAS.to_string(),
        }])
    }

    async fn connect_peer(
        &self,
        _public_key: PublicKey,
        _socket_addr: Option<NetAddress>,
    ) -> Result<()> {
        Ok(())
    }

    async fn disconnect_peer(&self, _public_key: PublicKey) -> Result<()> {
        Ok(())
    }

    async fn close_channel(
        &self,
        _channel_id: &[u8; 32],
        _counterparty_node_id: &PublicKey,
    ) -> Result<()> {
        Ok(())
    }

    fn get_node(&self, _node_id: &NodeId) -> Option<NodeInfo> {
        let mut alias = [0u8; 32];
        alias[..TEST_ALIAS.len()].copy_from_slice(TEST_ALIAS.as_bytes());
        let announcement = NodeAnnouncementInfo {
            features: Features::empty(),
            last_update: 21000000,
            rgb: [1, 2, 3],
            alias: NodeAlias(alias),
            announcement_message: None,
        };
        Some(NodeInfo {
            channels: vec![],
            announcement_info: Some(announcement),
        })
    }

    fn nodes(&self) -> IndexedMap<NodeId, NodeInfo> {
        let mut nodes = IndexedMap::new();
        let node_id = NodeId::from_pubkey(&self.public_key);
        nodes.insert(node_id, self.get_node(&node_id).unwrap());
        nodes
    }

    fn get_channel(&self, _channel_id: u64) -> Option<ChannelInfo> {
        None
    }

    fn channels(&self) -> IndexedMap<u64, ChannelInfo> {
        IndexedMap::new()
    }

    fn user_config(&self) -> UserConfig {
        UserConfig::default()
    }

    async fn generate_invoice(
        &self,
        _label: String,
        _amount: Option<u64>,
        _description: String,
        _expiry: Option<u32>,
    ) -> Result<Invoice> {
        Ok(self.invoice.clone())
    }

    async fn pay_invoice(&self, invoice: Invoice, label: Option<String>) -> Result<Payment> {
        let mut payment = Payment::of_invoice_outbound(&invoice, label);
        payment.succeeded(Some(PaymentPreimage([1u8; 32])), Some(2323));
        Ok(payment)
    }

    async fn list_payments(
        &self,
        _bolt11: Option<Invoice>,
        _direction: Option<PaymentDirection>,
    ) -> Result<Vec<Payment>> {
        Ok(vec![self.payment.clone()])
    }

    async fn list_invoices(&self, _label: Option<String>) -> Result<Vec<Invoice>> {
        Ok(vec![self.invoice.clone()])
    }

    async fn keysend_payment(&self, _payee: NodeId, _amount: MillisatAmount) -> Result<Payment> {
        Ok(self.payment.clone())
    }

    async fn lsp_list_protocols(&self, _node_id: Option<PublicKey>) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    async fn estimated_channel_liquidity_range(
        &self,
        _scid: u64,
        _target: &NodeId,
    ) -> Result<Option<(u64, u64)>> {
        Ok(Some((100, 100000)))
    }
}
