use std::net::SocketAddr;

use anyhow::Result;
use async_trait::async_trait;
use bitcoin::{secp256k1::PublicKey, Network, Transaction, Txid};
use lightning::{ln::channelmanager::ChannelDetails, util::config::UserConfig};

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

    fn alias_of(&self, node_id: PublicKey) -> Option<String>;

    fn addresses(&self) -> Vec<String>;

    async fn list_peers(&self) -> Result<Vec<Peer>>;

    async fn connect_peer(
        &self,
        public_key: PublicKey,
        socket_addr: Option<SocketAddr>,
    ) -> Result<()>;

    async fn disconnect_peer(&self, public_key: PublicKey) -> Result<()>;

    async fn open_channel(
        &self,
        their_network_key: PublicKey,
        channel_value_satoshis: u64,
        push_msat: Option<u64>,
        override_config: Option<UserConfig>,
    ) -> Result<OpenChannelResult>;
}

pub struct Peer {
    pub public_key: PublicKey,
    pub socket_addr: SocketAddr,
    pub status: PeerStatus,
    pub alias: String,
}

pub enum PeerStatus {
    Connected,
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
