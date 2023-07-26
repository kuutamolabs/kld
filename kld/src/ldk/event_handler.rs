use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::anyhow;

use bitcoin::blockdata::locktime::PackedLockTime;
use bitcoin::secp256k1::Secp256k1;

use crate::database::payment::Payment;
use crate::database::spendable_output::{SpendableOutput, SpendableOutputStatus};
use crate::database::{LdkDatabase, WalletDatabase};
use crate::settings::Settings;
use hex::ToHex;
use lightning::chain::chaininterface::{BroadcasterInterface, ConfirmationTarget, FeeEstimator};
use lightning::events::{Event, HTLCDestination, PathFailure, PaymentPurpose};
use lightning::routing::gossip::NodeId;
use lightning::sign::KeysManager;
use lightning_block_sync::BlockSource;
use log::{error, info, warn};
use rand::{thread_rng, Rng};
use tokio::runtime::Handle;

use crate::bitcoind::BitcoindClient;
use crate::ldk::ldk_error;
use crate::wallet::{Wallet, WalletInterface};

use super::controller::AsyncAPIRequests;
use super::peer_manager::PeerManager;
use super::{ChannelManager, NetworkGraph};

pub(crate) struct EventHandler {
    channel_manager: Arc<ChannelManager>,
    bitcoind_client: Arc<BitcoindClient>,
    keys_manager: Arc<KeysManager>,
    network_graph: Arc<NetworkGraph>,
    wallet: Arc<Wallet<WalletDatabase, BitcoindClient>>,
    ldk_database: Arc<LdkDatabase>,
    peer_manager: Arc<PeerManager>,
    async_api_requests: Arc<AsyncAPIRequests>,
    settings: Arc<Settings>,
    runtime_handle: Handle,
}

impl EventHandler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        channel_manager: Arc<ChannelManager>,
        bitcoind_client: Arc<BitcoindClient>,
        keys_manager: Arc<KeysManager>,
        network_graph: Arc<NetworkGraph>,
        wallet: Arc<Wallet<WalletDatabase, BitcoindClient>>,
        database: Arc<LdkDatabase>,
        peer_manager: Arc<PeerManager>,
        async_api_requests: Arc<AsyncAPIRequests>,
        settings: Arc<Settings>,
    ) -> EventHandler {
        EventHandler {
            channel_manager,
            bitcoind_client,
            keys_manager,
            network_graph,
            wallet,
            ldk_database: database,
            peer_manager,
            async_api_requests,
            settings,
            runtime_handle: Handle::current(),
        }
    }
}

impl lightning::events::EventHandler for EventHandler {
    fn handle_event(&self, event: lightning::events::Event) {
        tokio::task::block_in_place(move || {
            self.runtime_handle.block_on(self.handle_event_async(event))
        })
    }
}

impl EventHandler {
    pub async fn handle_event_async(&self, event: lightning::events::Event) {
        match event {
            Event::FundingGenerationReady {
                temporary_channel_id,
                counterparty_node_id,
                channel_value_satoshis,
                output_script,
                user_channel_id,
            } => {
                let (fee_rate, respond) = match self
                    .async_api_requests
                    .funding_transactions
                    .get(&user_channel_id)
                    .await
                {
                    Some(fee_rate) => fee_rate,
                    None => {
                        error!(
                            "Can't find funding transaction for user_channel_id {user_channel_id}"
                        );
                        return;
                    }
                };
                let funding_tx =
                    match self
                        .wallet
                        .fund_tx(&output_script, &channel_value_satoshis, fee_rate)
                    {
                        Ok(tx) => tx,
                        Err(e) => {
                            error!("Event::FundingGenerationReady: {e}");
                            respond(Err(e));
                            return;
                        }
                    };

                // Give the funding transaction back to LDK for opening the channel.
                if let Err(e) = self
                    .channel_manager
                    .funding_transaction_generated(
                        &temporary_channel_id,
                        &counterparty_node_id,
                        funding_tx.clone(),
                    )
                    .map_err(ldk_error)
                {
                    error!("Event::FundingGenerationReady: {e}");
                    respond(Err(e));
                    return;
                }
                info!("EVENT: Channel with user channel id {user_channel_id} has been funded");
                respond(Ok(funding_tx))
            }
            Event::ChannelPending {
                channel_id,
                user_channel_id,
                former_temporary_channel_id: _,
                counterparty_node_id,
                funding_txo,
            } => {
                info!(
                    "EVENT: Channel {} - {user_channel_id} with counterparty {counterparty_node_id} is pending. OutPoint: {funding_txo}",
                    channel_id.encode_hex::<String>(),
                );
            }
            Event::ChannelReady {
                channel_id,
                user_channel_id,
                counterparty_node_id,
                channel_type: _,
            } => {
                info!(
                    "EVENT: Channel {} - {user_channel_id} with counterparty {counterparty_node_id} is ready to use.",
                    channel_id.encode_hex::<String>(),
                );
                let mut alias = [0; 32];
                alias[..self.settings.node_alias.len()]
                    .copy_from_slice(self.settings.node_alias.as_bytes());
                let addresses: Vec<lightning::ln::msgs::NetAddress> = self
                    .settings
                    .public_addresses
                    .clone()
                    .into_iter()
                    .map(|a| a.inner())
                    .collect();
                info!("Broadcasting node announcement message");
                self.peer_manager
                    .broadcast_node_announcement([0; 3], alias, addresses);
            }
            Event::ChannelClosed {
                channel_id,
                reason,
                user_channel_id,
            } => {
                info!(
                    "EVENT: Channel {}: {reason}.",
                    channel_id.encode_hex::<String>()
                );
                self.async_api_requests
                    .funding_transactions
                    .respond(
                        &user_channel_id,
                        Err(anyhow!("Channel closed due to {reason}")),
                    )
                    .await;
            }
            Event::DiscardFunding {
                channel_id,
                transaction,
            } => {
                info!(
                    "EVENT: Funding discarded for channel: {}, txid: {}",
                    channel_id.encode_hex::<String>(),
                    transaction.txid()
                )
            }
            Event::OpenChannelRequest { .. } => {
                unreachable!(
                    "This event will not fire as we do not manually accept inbound channels."
                )
            }
            Event::PaymentClaimable {
                payment_hash,
                purpose,
                amount_msat,
                receiver_node_id: _,
                via_channel_id,
                via_user_channel_id: _,
                onion_fields: _,
                claim_deadline,
                ..
            } => {
                info!(
                    "EVENT: Payment claimable with hash {} of {} millisatoshis {} {}",
                    payment_hash.0.encode_hex::<String>(),
                    amount_msat,
                    if let Some(channel_id) = via_channel_id {
                        format!("via channel ID {} ", channel_id.encode_hex::<String>())
                    } else {
                        String::new()
                    },
                    if let Some(deadline) = claim_deadline {
                        format!(
                            "with deadline {:?}",
                            SystemTime::UNIX_EPOCH + Duration::from_secs(deadline as u64)
                        )
                    } else {
                        String::new()
                    }
                );
                match purpose {
                    PaymentPurpose::InvoicePayment {
                        payment_preimage, ..
                    } => {
                        if let Some(payment_preimage) = payment_preimage {
                            self.channel_manager.claim_funds(payment_preimage);
                        }
                    }
                    PaymentPurpose::SpontaneousPayment(preimage) => {
                        self.channel_manager.claim_funds(preimage);
                    }
                };
            }
            Event::PaymentClaimed {
                payment_hash,
                purpose,
                amount_msat,
                receiver_node_id: _,
            } => {
                info!(
                    "EVENT: Payment claimed with hash {} of {} millisatoshis",
                    payment_hash.0.encode_hex::<String>(),
                    amount_msat,
                );
                let payment = match purpose {
                    PaymentPurpose::InvoicePayment {
                        payment_preimage,
                        payment_secret,
                    } => Payment::of_invoice_inbound(
                        payment_hash,
                        payment_preimage,
                        payment_secret,
                        amount_msat,
                    ),
                    PaymentPurpose::SpontaneousPayment(preimage) => {
                        Payment::spontaneous_inbound(payment_hash, preimage, amount_msat)
                    }
                };
                if let Err(e) = self.ldk_database.persist_payment(&payment).await {
                    error!(
                        "Failed to persist payment with hash {}: {e}",
                        payment_hash.0.encode_hex::<String>()
                    )
                }
            }
            Event::PaymentSent {
                payment_id,
                payment_preimage,
                payment_hash,
                fee_paid_msat,
            } => {
                info!(
                    "EVENT: Payment with hash {}{} sent successfully{}",
                    payment_hash.0.encode_hex::<String>(),
                    if let Some(id) = payment_id {
                        format!(" and ID {}", id.0.encode_hex::<String>())
                    } else {
                        "".to_string()
                    },
                    if let Some(fee) = fee_paid_msat {
                        format!(" with fee {fee} msat")
                    } else {
                        "".to_string()
                    },
                );
                match payment_id {
                    Some(id) => {
                        if let Some((mut payment, respond)) =
                            self.async_api_requests.payments.get(&id).await
                        {
                            payment.succeeded(Some(payment_preimage), fee_paid_msat);
                            respond(Ok(payment));
                        } else {
                            error!("Can't find payment for {}", id.0.encode_hex::<String>());
                        }
                    }
                    None => {
                        error!(
                            "Failed to update payment with hash {}",
                            payment_hash.0.encode_hex::<String>()
                        )
                    }
                }
            }
            Event::PaymentPathSuccessful {
                payment_id,
                payment_hash,
                path,
            } => {
                info!(
                    "EVENT: Payment path with {} hops successful for payment with ID {}{}",
                    path.hops.len(),
                    payment_id.0.encode_hex::<String>(),
                    payment_hash
                        .map(|h| format!(" and hash {}", h.0.encode_hex::<String>()))
                        .unwrap_or_default()
                );
            }
            Event::PaymentPathFailed {
                payment_id,
                payment_hash,
                payment_failed_permanently,
                failure,
                path,
                short_channel_id,
            } => {
                match failure {
                    PathFailure::InitialSend { err } => warn!("{}", ldk_error(err)),
                    PathFailure::OnPath { network_update } => {
                        if let Some(update) = network_update {
                            self.network_graph.handle_network_update(&update);
                        }
                    }
                };
                info!(
                    "EVENT: Payment path failed for payment with hash {}{}. Payment failed {} {}. Path: {:?}",
                    payment_hash.0.encode_hex::<String>(),
                    payment_id.map(|id| format!(" and ID {}", id.0.encode_hex::<String>())).unwrap_or_default(),
                    if payment_failed_permanently {
                        "permanently"
                    } else {
                        "temporarily"
                    },
                    if let Some(short_channel_id) = short_channel_id {
                        format!("along channel {}", short_channel_id)
                    } else {
                        "".to_string()
                    },
                    path
                );
            }
            Event::PaymentFailed {
                payment_id,
                payment_hash,
                reason,
            } => {
                info!(
                    "EVENT: Failed to send payment with ID {} and hash {}{}",
                    payment_id.0.encode_hex::<String>(),
                    payment_hash.0.encode_hex::<String>(),
                    reason
                        .map(|r| format!(" for reason {r:?}"))
                        .unwrap_or_default()
                );
                if let Some((mut payment, respond)) =
                    self.async_api_requests.payments.get(&payment_id).await
                {
                    payment.failed(reason);
                    respond(Ok(payment));
                } else {
                    error!(
                        "Can't find payment for {}",
                        payment_id.0.encode_hex::<String>()
                    );
                }
            }
            Event::PaymentForwarded {
                prev_channel_id,
                next_channel_id,
                fee_earned_msat,
                claim_from_onchain_tx,
                outbound_amount_forwarded_msat,
            } => {
                let read_only_network_graph = self.network_graph.read_only();
                let nodes = read_only_network_graph.nodes();
                let channels = self.channel_manager.list_channels();

                let node_str = |channel_id: &Option<[u8; 32]>| match channel_id {
                    None => String::new(),
                    Some(channel_id) => match channels.iter().find(|c| c.channel_id == *channel_id)
                    {
                        None => String::new(),
                        Some(channel) => {
                            match nodes.get(&NodeId::from_pubkey(&channel.counterparty.node_id)) {
                                None => "private node".to_string(),
                                Some(node) => match &node.announcement_info {
                                    None => "unnamed node".to_string(),
                                    Some(announcement) => {
                                        format!("node {}", announcement.alias)
                                    }
                                },
                            }
                        }
                    },
                };
                let channel_str = |channel_id: &Option<[u8; 32]>| {
                    channel_id
                        .map(|channel_id| {
                            format!(" with channel {}", channel_id.encode_hex::<String>())
                        })
                        .unwrap_or_default()
                };
                let from_prev_str = format!(
                    " from {}{}",
                    node_str(&prev_channel_id),
                    channel_str(&prev_channel_id)
                );
                let to_next_str = format!(
                    " to {}{}",
                    node_str(&next_channel_id),
                    channel_str(&next_channel_id)
                );

                let from_onchain_str = if claim_from_onchain_tx {
                    "from onchain downstream claim"
                } else {
                    "from HTLC fulfill message"
                };
                let amount_str = if let Some(amount) = outbound_amount_forwarded_msat {
                    format!("of amount {amount}")
                } else {
                    "of unknown amount".to_string()
                };
                let fee_str = if let Some(fee_earned) = fee_earned_msat {
                    format!(" earning {fee_earned} msat")
                } else {
                    "".to_string()
                };
                info!(
                    "EVENT: Forwarded payment{from_prev_str}{to_next_str} {amount_str},{fee_str} {from_onchain_str}",
                );
            }
            Event::ProbeSuccessful { .. } => {}
            Event::ProbeFailed { .. } => {}
            Event::HTLCHandlingFailed {
                prev_channel_id,
                failed_next_destination,
            } => {
                error!(
                    "EVENT: Failed handling HTLC from channel {}. {}",
                    prev_channel_id.encode_hex::<String>(),
                    match failed_next_destination {
                        HTLCDestination::NextHopChannel {
                            node_id: _,
                            channel_id,
                        } => format!("Next hop channel ID {}", channel_id.encode_hex::<String>()),
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
                        HTLCDestination::FailedPayment { payment_hash } => format!(
                            "Failed payment with hash {}",
                            payment_hash.0.encode_hex::<String>()
                        ),
                    }
                );
            }
            Event::PendingHTLCsForwardable { time_forwardable } => {
                let forwarding_channel_manager = self.channel_manager.clone();
                let min = time_forwardable.as_millis() as u64;
                tokio::spawn(async move {
                    let millis_to_sleep = thread_rng().gen_range(min..min * 5);
                    tokio::time::sleep(Duration::from_millis(millis_to_sleep)).await;
                    forwarding_channel_manager.process_pending_htlc_forwards();
                });
            }
            Event::SpendableOutputs { outputs } => {
                let mut spendable_outputs: Vec<SpendableOutput> =
                    outputs.into_iter().map(SpendableOutput::new).collect();
                for spendable_output in &spendable_outputs {
                    info!("EVENT: New {:?}", spendable_output);
                    self.persist_spendable_output(spendable_output.clone());
                }
                let destination_address = match self.wallet.new_address() {
                    Ok(a) => a,
                    Err(e) => {
                        error!("Could not get new address: {}", e);
                        return;
                    }
                };
                let output_descriptors = &spendable_outputs
                    .iter()
                    .map(|o| &o.descriptor)
                    .collect::<Vec<_>>();
                let tx_feerate = self
                    .bitcoind_client
                    .get_est_sat_per_1000_weight(ConfirmationTarget::HighPriority);

                let best_block_height = self
                    .bitcoind_client
                    .get_best_block()
                    .await
                    .map(|r| r.1.unwrap_or_default())
                    .unwrap_or_default();
                match self.keys_manager.spend_spendable_outputs(
                    output_descriptors,
                    Vec::new(),
                    destination_address.script_pubkey(),
                    tx_feerate,
                    Some(PackedLockTime(best_block_height)),
                    &Secp256k1::new(),
                ) {
                    Ok(spending_tx) => {
                        info!(
                            "Sending spendable output to {}",
                            destination_address.address
                        );
                        self.bitcoind_client.broadcast_transactions(&[&spending_tx]);
                        for spendable_output in spendable_outputs.iter_mut() {
                            spendable_output.status = SpendableOutputStatus::Spent;
                            self.persist_spendable_output(spendable_output.clone());
                        }
                    }
                    Err(_) => {
                        error!("Failed to build spending transaction");
                    }
                };
            }
            Event::HTLCIntercepted {
                intercept_id: _,
                requested_next_hop_scid: _,
                payment_hash: _,
                inbound_amount_msat: _,
                expected_outbound_amount_msat: _,
            } => {}
            Event::BumpTransaction(_) => todo!(),
        }
    }

    fn persist_spendable_output(&self, spendable_output: SpendableOutput) {
        let database = self.ldk_database.clone();
        self.runtime_handle.spawn(async move {
            if let Err(e) = database.persist_spendable_output(spendable_output).await {
                error!("{e}");
            }
        });
    }
}
