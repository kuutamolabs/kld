use anyhow::Result;
use api::FeeRate;
use async_trait::async_trait;
use bitcoin::{secp256k1::PublicKey, Network, Transaction, Txid};
use lightning::{
    ln::{channelmanager::ChannelDetails, msgs::NetAddress},
    routing::gossip::{ChannelInfo, NodeId, NodeInfo},
    util::{config::UserConfig, indexed_map::IndexedMap},
};

use crate::database::{
    invoice::Invoice,
    payment::{MillisatAmount, Payment},
};

use super::net_utils::PeerAddress;

#[async_trait]
pub trait LightningInterface: Send + Sync {
    fn alias(&self) -> String;

    fn identity_pubkey(&self) -> PublicKey;

    async fn synced(&self) -> Result<bool>;

    fn sign(&self, message: &[u8]) -> Result<String>;

    fn network(&self) -> Network;

    fn num_active_channels(&self) -> usize;

    fn num_inactive_channels(&self) -> usize;

    fn num_pending_channels(&self) -> usize;

    fn graph_num_nodes(&self) -> usize;

    fn graph_num_channels(&self) -> usize;

    fn num_peers(&self) -> usize;

    fn wallet_balance(&self) -> u64;

    fn list_channels(&self) -> Vec<ChannelDetails>;

    fn set_channel_fee(
        &self,
        counterparty_node_id: &PublicKey,
        channel_id: &[[u8; 32]],
        forwarding_fee_proportional_millionths: Option<u32>,
        forwarding_fee_base_msat: Option<u32>,
    ) -> Result<(u32, u32)>;

    fn alias_of(&self, node_id: &PublicKey) -> Option<String>;

    fn public_addresses(&self) -> Vec<String>;

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
        fee_rate: Option<FeeRate>,
        override_config: Option<UserConfig>,
    ) -> Result<OpenChannelResult>;

    async fn close_channel(
        &self,
        channel_id: &[u8; 32],
        counterparty_node_id: &PublicKey,
    ) -> Result<()>;

    fn get_node(&self, node_id: &NodeId) -> Option<NodeInfo>;

    fn nodes(&self) -> IndexedMap<NodeId, NodeInfo>;

    fn get_channel(&self, channel_id: u64) -> Option<ChannelInfo>;

    fn channels(&self) -> IndexedMap<u64, ChannelInfo>;

    fn user_config(&self) -> UserConfig;

    async fn pay_invoice(&self, invoice: Invoice, label: Option<String>) -> Result<Payment>;

    async fn keysend_payment(&self, payee: NodeId, amount: MillisatAmount) -> Result<Payment>;

    async fn generate_invoice(
        &self,
        label: String,
        amount: Option<u64>,
        description: String,
        expiry: Option<u32>,
    ) -> Result<Invoice>;

    async fn list_invoices(&self, label: Option<String>) -> Result<Vec<Invoice>>;
}

pub struct Peer {
    pub public_key: PublicKey,
    pub net_address: Option<NetAddress>,
    pub status: PeerStatus,
    pub alias: String,
}

#[derive(Copy, Clone, PartialEq, Default)]
pub enum PeerStatus {
    Connected,
    #[default]
    Disconnected,
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
