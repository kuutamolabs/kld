use std::str::FromStr;

use anyhow::Result;
use api::FeeRate;
use async_trait::async_trait;
use bitcoin::{consensus::deserialize, hashes::Hash, secp256k1::PublicKey, Network, Txid};
use hex::FromHex;
use kld::ldk::{net_utils::PeerAddress, LightningInterface, OpenChannelResult, Peer, PeerStatus};
use lightning::{
    chain::transaction::OutPoint,
    ln::{
        channelmanager::{ChannelCounterparty, ChannelDetails},
        features::{Features, InitFeatures},
        msgs::NetAddress,
    },
    routing::gossip::{NodeAlias, NodeAnnouncementInfo, NodeId, NodeInfo},
    util::{config::UserConfig, indexed_map::IndexedMap},
};

use super::{TEST_ALIAS, TEST_PUBLIC_KEY, TEST_SHORT_CHANNEL_ID, TEST_TX};

pub struct MockLightning {
    pub num_peers: usize,
    pub num_nodes: usize,
    pub num_channels: usize,
    pub wallet_balance: u64,
    pub channels: Vec<ChannelDetails>,
    pub public_key: PublicKey,
    pub ipv4_address: NetAddress,
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
                txid: Txid::all_zeros(),
                index: 2,
            }),
            channel_type: None,
            short_channel_id: Some(TEST_SHORT_CHANNEL_ID),
            outbound_scid_alias: None,
            inbound_scid_alias: None,
            channel_value_satoshis: 1000000,
            unspendable_punishment_reserve: Some(10000),
            user_channel_id: 3434232,
            balance_msat: 10001,
            outbound_capacity_msat: 100000,
            next_outbound_htlc_limit_msat: 500,
            inbound_capacity_msat: 200000,
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
        };
        let ipv4_address = NetAddress::IPv4 {
            addr: [127, 0, 0, 1],
            port: 5555,
        };
        Self {
            num_peers: 5,
            num_nodes: 6,
            num_channels: 7,
            wallet_balance: 8,
            channels: vec![channel],
            public_key,
            ipv4_address,
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

    fn graph_num_nodes(&self) -> usize {
        self.num_nodes
    }

    fn graph_num_channels(&self) -> usize {
        self.num_channels
    }

    fn block_height(&self) -> Result<u64> {
        Ok(50000)
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

    fn public_addresses(&self) -> Vec<String> {
        vec![
            "127.0.0.1:2324".to_string(),
            "194.454.23.2:2020".to_string(),
        ]
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
        _socket_addr: Option<PeerAddress>,
    ) -> Result<()> {
        Ok(())
    }

    async fn disconnect_peer(&self, _public_key: PublicKey) -> Result<()> {
        Ok(())
    }

    fn close_channel(
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
            addresses: vec![self.ipv4_address.clone()],
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

    fn user_config(&self) -> UserConfig {
        UserConfig::default()
    }
}
