pub mod channel_utils;
pub mod controller;
mod event_handler;
pub mod lightning_interface;
mod peer_manager;

use std::sync::{Arc, RwLock};

use crate::database::LdkDatabase;
use crate::logger::KldLogger;
use anyhow::anyhow;
use bitcoin::secp256k1::PublicKey;
use lightning::ln::peer_handler::CustomMessageHandler;
use lightning::{
    chain::{chainmonitor, Filter},
    events::HTLCDestination,
    ln::{
        channelmanager::{PaymentSendFailure, RetryableSendFailure, SimpleArcChannelManager},
        features::{InitFeatures, NodeFeatures},
        msgs::{DecodeError, LightningError},
        wire::CustomMessageReader,
    },
    onion_message::SimpleArcOnionMessenger,
    routing::{
        gossip,
        router::DefaultRouter,
        scoring::{ProbabilisticScorer, ProbabilisticScoringFeeParameters},
    },
    sign::{InMemorySigner, KeysManager},
    util::errors::APIError,
};
use lightning_invoice::SignOrCreationError;

pub use controller::Controller;
pub use lightning_interface::{LightningInterface, OpenChannelResult, Peer, PeerStatus};
use log::warn;

use crate::bitcoind::BitcoindClient;

/// The minimum feerate we are allowed to send, as specify by LDK (sats/kwu).
pub static MIN_FEERATE: u32 = 2000;

pub type NetworkGraph = gossip::NetworkGraph<Arc<KldLogger>>;

pub(crate) type ChainMonitor = chainmonitor::ChainMonitor<
    InMemorySigner,
    Arc<dyn Filter + Send + Sync>,
    Arc<BitcoindClient>,
    Arc<BitcoindClient>,
    Arc<KldLogger>,
    Arc<LdkDatabase>,
>;

pub(crate) type LiquidityManager = lightning_liquidity::LiquidityManager<
    Arc<KeysManager>,
    Arc<ChannelManager>,
    Arc<dyn Filter + Send + Sync>,
>;

pub(crate) struct KuutamoCustomMessageHandler {
    liquidity_manager: LiquidityManager,
}

impl lightning::ln::wire::CustomMessageReader for KuutamoCustomMessageHandler {
    type CustomMessage = <LiquidityManager as CustomMessageReader>::CustomMessage;
    fn read<RD: lightning::io::Read>(
        &self,
        message_type: u16,
        buffer: &mut RD,
    ) -> Result<Option<Self::CustomMessage>, lightning::ln::msgs::DecodeError> {
        self.liquidity_manager.read(message_type, buffer)
    }
}

impl CustomMessageHandler for KuutamoCustomMessageHandler {
    fn handle_custom_message(
        &self,
        msg: Self::CustomMessage,
        sender_node_id: &PublicKey,
    ) -> Result<(), LightningError> {
        self.liquidity_manager
            .handle_custom_message(msg, sender_node_id)
    }

    fn get_and_clear_pending_msg(&self) -> Vec<(PublicKey, Self::CustomMessage)> {
        self.liquidity_manager.get_and_clear_pending_msg()
    }

    fn provided_node_features(&self) -> NodeFeatures {
        self.liquidity_manager.provided_node_features()
    }

    fn provided_init_features(&self, their_node_id: &PublicKey) -> InitFeatures {
        self.liquidity_manager.provided_init_features(their_node_id)
    }
}

pub(crate) type ChannelManager =
    SimpleArcChannelManager<ChainMonitor, BitcoindClient, BitcoindClient, KldLogger>;

pub(crate) type OnionMessenger =
    SimpleArcOnionMessenger<ChainMonitor, BitcoindClient, BitcoindClient, KldLogger>;

pub type Scorer = ProbabilisticScorer<Arc<NetworkGraph>, Arc<KldLogger>>;

pub(crate) type KldRouter = DefaultRouter<
    Arc<NetworkGraph>,
    Arc<KldLogger>,
    Arc<RwLock<Scorer>>,
    ProbabilisticScoringFeeParameters,
    Scorer,
>;

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

pub fn lightning_error(error: LightningError) -> anyhow::Error {
    anyhow!(error.err)
}

pub fn retryable_send_failure(error: RetryableSendFailure) -> anyhow::Error {
    match error {
        RetryableSendFailure::PaymentExpired => {
            anyhow!("Payment failure: payment has expired")
        }
        RetryableSendFailure::RouteNotFound => {
            anyhow!("Payment failure: route not found")
        }
        RetryableSendFailure::DuplicatePayment => {
            anyhow!("Payment failure: duplicate payment")
        }
    }
}

pub fn sign_or_creation_error(error: SignOrCreationError) -> anyhow::Error {
    match error {
        SignOrCreationError::SignError(()) => anyhow!("Error signing invoice"),
        SignOrCreationError::CreationError(e) => anyhow!("Error creating invoice: {e}"),
    }
}

pub fn payment_send_failure(error: PaymentSendFailure) -> anyhow::Error {
    match error {
        PaymentSendFailure::ParameterError(api_error) => ldk_error(api_error),
        PaymentSendFailure::PathParameterError(results) => {
            for result in results {
                if let Err(e) = result {
                    warn!("{}", ldk_error(e));
                }
            }
            anyhow!("Payment failure: Path parameter error. Check logs for more details.")
        }
        PaymentSendFailure::AllFailedResendSafe(errors) => {
            for e in errors {
                warn!("{}", ldk_error(e));
            }
            anyhow!("Payment failure: All failed, resend safe. Check logs for more details.")
        }
        PaymentSendFailure::DuplicatePayment => anyhow!("Payment failed: Duplicate Payment"),
        PaymentSendFailure::PartialFailure {
            results,
            failed_paths_retry: _,
            payment_id: _,
        } => {
            for result in results {
                if let Err(e) = result {
                    warn!("{}", ldk_error(e));
                }
            }
            anyhow!("Payment failed: Partial failure. Check logs for more details.")
        }
    }
}

pub fn decode_error(error: DecodeError) -> anyhow::Error {
    match error {
        DecodeError::UnknownVersion => anyhow!("Unknown version"),
        DecodeError::UnknownRequiredFeature => anyhow!("Unknown required feature"),
        DecodeError::InvalidValue => anyhow!("Invalid value"),
        DecodeError::ShortRead => anyhow!("Short read"),
        DecodeError::BadLengthDescriptor => anyhow!("Bad length descriptor"),
        DecodeError::Io(e) => anyhow!(e),
        DecodeError::UnsupportedCompression => anyhow!("Unsupported compression"),
    }
}

pub fn htlc_destination_to_string(destination: &HTLCDestination) -> String {
    match destination {
        HTLCDestination::NextHopChannel {
            node_id: _,
            channel_id,
        } => format!("Next hop channel ID {}", hex::encode(channel_id.0)),
        HTLCDestination::UnknownNextHop {
            requested_forward_scid,
        } => format!(
            "Unknown next hop to requested SCID {}",
            requested_forward_scid
        ),
        HTLCDestination::InvalidForward {
            requested_forward_scid,
        } => format!(
            "Invalid forward to requested SCID {}",
            requested_forward_scid
        ),
        HTLCDestination::FailedPayment { payment_hash } => {
            format!("Failed payment with hash {}", hex::encode(payment_hash.0))
        }
    }
}
