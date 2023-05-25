pub mod channel_utils;
pub mod controller;
mod event_handler;
pub mod lightning_interface;
pub mod net_utils;
mod peer_manager;

use std::sync::Arc;

use crate::database::LdkDatabase;
use crate::logger::KldLogger;
use lightning::{
    chain::{chainmonitor, keysinterface::InMemorySigner, Filter},
    ln::{channelmanager::SimpleArcChannelManager, peer_handler::SimpleArcPeerManager},
    onion_message::SimpleArcOnionMessenger,
    routing::gossip,
    util::errors::APIError,
};
use lightning_net_tokio::SocketDescriptor;

pub use controller::Controller;
pub use lightning_interface::{LightningInterface, OpenChannelResult, Peer, PeerStatus};

use crate::bitcoind::{BitcoindClient, BitcoindUtxoLookup};

/// The minimum feerate we are allowed to send, as specify by LDK (sats/kwu).
pub static MIN_FEERATE: u32 = 253;

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

pub fn ldk_error(error: APIError) -> anyhow::Error {
    anyhow::Error::msg(match error {
        APIError::APIMisuseError { ref err } => format!("Misuse error: {err}"),
        APIError::FeeRateTooHigh {
            ref err,
            ref feerate,
        } => format!("{err} feerate: {feerate}"),
        APIError::InvalidRoute { ref err } => format!("Invalid route provided: {err}"),
        APIError::ChannelUnavailable { ref err } => format!("Channel unavailable: {err}"),
        APIError::MonitorUpdateInProgress => {
            "Client indicated a channel monitor update is in progress but not yet complete"
                .to_string()
        }
        APIError::IncompatibleShutdownScript { ref script } => {
            format!("Provided a scriptpubkey format not accepted by peer: {script}")
        }
    })
}
