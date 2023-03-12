pub mod channel_utils;
pub mod controller;
mod event_handler;
pub mod lightning_interface;
pub mod net_utils;
mod payment_info;
mod peer_manager;

use std::sync::Arc;

use database::ldk_database::LdkDatabase;
use lightning::{
    chain::{chainmonitor, keysinterface::InMemorySigner, Filter},
    ln::{channelmanager::SimpleArcChannelManager, peer_handler::SimpleArcPeerManager},
    onion_message::SimpleArcOnionMessenger,
    routing::gossip,
};
use lightning_net_tokio::SocketDescriptor;
use logger::KldLogger;

pub use controller::Controller;
pub use lightning_interface::{LightningInterface, OpenChannelResult, Peer, PeerStatus};

use crate::bitcoind::{BitcoindClient, BitcoindUtxoLookup};

pub type NetworkGraph = gossip::NetworkGraph<Arc<KldLogger>>;

pub(crate) type LdkPeerManager = SimpleArcPeerManager<
    SocketDescriptor,
    ChainMonitor,
    BitcoindClient,
    BitcoindClient,
    BitcoindUtxoLookup,
    KldLogger,
>;

pub(crate) type ChainMonitor = chainmonitor::ChainMonitor<
    InMemorySigner,
    Arc<dyn Filter + Send + Sync>,
    Arc<BitcoindClient>,
    Arc<BitcoindClient>,
    Arc<KldLogger>,
    Arc<LdkDatabase>,
>;

pub(crate) type ChannelManager =
    SimpleArcChannelManager<ChainMonitor, BitcoindClient, BitcoindClient, KldLogger>;

pub(crate) type OnionMessenger = SimpleArcOnionMessenger<KldLogger>;
