pub mod channel_utils;
pub mod controller;
mod event_handler;
pub mod lightning_interface;
mod peer_manager;

use std::sync::{Arc, RwLock};

use crate::database::LdkDatabase;
use crate::logger::KldLogger;
use anyhow::anyhow;
use bitcoin::hashes::hex::ToHex;
use lightning::{
    chain::{chainmonitor, Filter},
    events::HTLCDestination,
    ln::{
        channelmanager::{PaymentSendFailure, RetryableSendFailure, SimpleArcChannelManager},
        msgs::{DecodeError, LightningError},
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

pub(crate) type LiquidityManager = ldk_lsp_client::LiquidityManager<
    Arc<KeysManager>,
    Arc<ChainMonitor>,
    Arc<BitcoindClient>,
    Arc<BitcoindClient>,
    Arc<KldRouter>,
    Arc<KeysManager>,
    Arc<KldLogger>,
    Arc<KeysManager>,
    Arc<dyn Filter + Send + Sync>,
>;

pub(crate) type ChannelManager =
    SimpleArcChannelManager<ChainMonitor, BitcoindClient, BitcoindClient, KldLogger>;

pub(crate) type OnionMessenger = SimpleArcOnionMessenger<KldLogger>;

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
        } => format!("Next hop channel ID {}", channel_id.to_hex()),
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
            format!("Failed payment with hash {}", payment_hash.0.to_hex())
        }
    }
}
