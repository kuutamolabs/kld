use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;

use bitcoin::secp256k1::Secp256k1;

use crate::database::payment::{MillisatAmount, Payment};
use crate::database::{LdkDatabase, WalletDatabase};
use hex::ToHex;
use lightning::chain::chaininterface::{BroadcasterInterface, ConfirmationTarget, FeeEstimator};
use lightning::chain::keysinterface::KeysManager;
use lightning::events::{Event, PaymentPurpose};
use lightning::routing::gossip::NodeId;
use log::{error, info};
use rand::{thread_rng, Rng};
use tokio::runtime::Handle;

use crate::bitcoind::BitcoindClient;
use crate::ldk::ldk_error;
use crate::wallet::{Wallet, WalletInterface};

use super::controller::AsyncAPIRequests;
use super::{ChannelManager, NetworkGraph};

pub(crate) struct EventHandler {
    channel_manager: Arc<ChannelManager>,
    bitcoind_client: Arc<BitcoindClient>,
    keys_manager: Arc<KeysManager>,
    network_graph: Arc<NetworkGraph>,
    wallet: Arc<Wallet<WalletDatabase, BitcoindClient>>,
    ldk_database: Arc<LdkDatabase>,
    async_api_requests: Arc<AsyncAPIRequests>,
    runtime_handle: Handle,
}

impl EventHandler {
    pub fn new(
        channel_manager: Arc<ChannelManager>,
        bitcoind_client: Arc<BitcoindClient>,
        keys_manager: Arc<KeysManager>,
        network_graph: Arc<NetworkGraph>,
        wallet: Arc<Wallet<WalletDatabase, BitcoindClient>>,
        database: Arc<LdkDatabase>,
        async_api_requests: Arc<AsyncAPIRequests>,
    ) -> EventHandler {
        EventHandler {
            channel_manager,
            bitcoind_client,
            keys_manager,
            network_graph,
            wallet,
            ldk_database: database,
            async_api_requests,
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
                via_channel_id: _,
                via_user_channel_id: _,
                onion_fields: _,
                claim_deadline: _,
            } => {
                info!(
                    "EVENT: Payment claimable with hash {} of {} millisatoshis",
                    payment_hash.0.encode_hex::<String>(),
                    amount_msat,
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
                    "EVENT: Payment claimed with payment hash {} of {} millisatoshis",
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
                        MillisatAmount(amount_msat),
                    ),
                    PaymentPurpose::SpontaneousPayment(preimage) => Payment::spontaneous_inbound(
                        payment_hash,
                        preimage,
                        MillisatAmount(amount_msat),
                    ),
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
                    "EVENT: Payment with id {:?} sent successfully with fee {}",
                    payment_id,
                    if let Some(fee) = fee_paid_msat {
                        format!(" (fee {fee} msat)")
                    } else {
                        "".to_string()
                    },
                );
                match payment_id {
                    Some(id) => {
                        if let Some((mut payment, respond)) =
                            self.async_api_requests.payments.get(&id).await
                        {
                            payment.succeeded(
                                Some(payment_preimage),
                                fee_paid_msat.map(MillisatAmount),
                            );
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
                payment_hash: _,
                path: _,
            } => {
                info!(
                    "EVENT: Payment path successful for payment with ID {}",
                    payment_id.0.encode_hex::<String>()
                );
            }
            Event::PaymentPathFailed {
                payment_id: _,
                payment_hash,
                payment_failed_permanently: _,
                failure: _,
                path: _,
                short_channel_id: _,
            } => {
                info!(
                    "EVENT: Payment path failed for payment with hash {}",
                    payment_hash.0.encode_hex::<String>()
                );
            }
            Event::PaymentFailed {
                payment_id,
                payment_hash: _,
                reason,
            } => {
                info!("EVENT: Failed to send payment {payment_id:?}: {reason:?}",);
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
                    format!("earning {fee_earned} msat")
                } else {
                    "claimed onchain".to_string()
                };
                info!(
                    "EVENT: Forwarded payment{from_prev_str}{to_next_str} {amount_str}, earning {fee_str} msat {from_onchain_str}",
                );
            }
            Event::ProbeSuccessful { .. } => {}
            Event::ProbeFailed { .. } => {}
            Event::HTLCHandlingFailed {
                prev_channel_id,
                failed_next_destination,
            } => {
                error!(
                    "EVENT: Failed handling HTLC from channel {} to {:?}",
                    prev_channel_id.encode_hex::<String>(),
                    failed_next_destination
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
                let destination_address = match self.wallet.new_address() {
                    Ok(a) => a,
                    Err(e) => {
                        error!("Could not get new address: {}", e);
                        return;
                    }
                };
                let output_descriptors = &outputs.iter().collect::<Vec<_>>();
                let tx_feerate = self
                    .bitcoind_client
                    .get_est_sat_per_1000_weight(ConfirmationTarget::Normal);
                match self.keys_manager.spend_spendable_outputs(
                    output_descriptors,
                    Vec::new(),
                    destination_address.script_pubkey(),
                    tx_feerate,
                    &Secp256k1::new(),
                ) {
                    Ok(spending_tx) => {
                        info!(
                            "EVENT: Sending spendable output to {}",
                            destination_address.address
                        );
                        self.bitcoind_client.broadcast_transaction(&spending_tx)
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
        }
    }
}
