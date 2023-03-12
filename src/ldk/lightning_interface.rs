use anyhow::Result;
use async_trait::async_trait;
use bitcoin::{secp256k1::PublicKey, Network, Transaction, Txid};
use lightning::{
    ln::{channelmanager::ChannelDetails, msgs::NetAddress},
    routing::gossip::{NodeId, NodeInfo},
    util::{config::UserConfig, indexed_map::IndexedMap},
};

use super::net_utils::PeerAddress;

#[async_trait]
pub trait LightningInterface {
    fn alias(&self) -> String;

    fn block_height(&self) -> Result<u64>;

    fn identity_pubkey(&self) -> PublicKey;

    fn network(&self) -> Network;

    fn num_active_channels(&self) -> usize;

    fn num_inactive_channels(&self) -> usize;

    fn num_pending_channels(&self) -> usize;

    fn graph_num_nodes(&self) -> usize;

    fn graph_num_channels(&self) -> usize;

    fn num_peers(&self) -> usize;

    fn wallet_balance(&self) -> u64;

    fn version(&self) -> String;

    fn list_channels(&self) -> Vec<ChannelDetails>;

    fn set_channel_fee(
        &self,
        counterparty_node_id: &PublicKey,
        channel_id: &[[u8; 32]],
        forwarding_fee_proportional_millionths: Option<u32>,
        forwarding_fee_base_msat: Option<u32>,
    ) -> Result<(u32, u32)>;

    fn alias_of(&self, node_id: &PublicKey) -> Option<String>;

    fn addresses(&self) -> Vec<String>;

    async fn list_peers(&self) -> Result<Vec<Peer>>;

    async fn connect_peer(
        &self,
        public_key: PublicKey,
        socket_addr: Option<PeerAddress>,
    ) -> Result<()>;

    async fn disconnect_peer(&self, public_key: PublicKey) -> Result<()>;

    async fn open_channel(
        &self,
        their_network_key: PublicKey,
        channel_value_satoshis: u64,
        push_msat: Option<u64>,
        override_config: Option<UserConfig>,
    ) -> Result<OpenChannelResult>;

    fn close_channel(&self, channel_id: &[u8; 32], counterparty_node_id: &PublicKey) -> Result<()>;

    fn get_node(&self, node_id: &NodeId) -> Option<NodeInfo>;

    fn nodes(&self) -> IndexedMap<NodeId, NodeInfo>;

    fn user_config(&self) -> UserConfig;
}

pub struct Peer {
    pub public_key: PublicKey,
    pub net_address: Option<NetAddress>,
    pub status: PeerStatus,
    pub alias: String,
}

#[derive(Copy, Clone, PartialEq)]
pub enum PeerStatus {
    Connected,
    Disconnected,
}

impl Default for PeerStatus {
    fn default() -> Self {
        PeerStatus::Disconnected
    }
}

impl ToString for PeerStatus {
    fn to_string(&self) -> String {
        match self {
            PeerStatus::Connected => "connected",
            PeerStatus::Disconnected => "disconnected",
        }
        .to_owned()
    }
}

pub struct OpenChannelResult {
    pub transaction: Transaction,
    pub txid: Txid,
    pub channel_id: [u8; 32],
}
