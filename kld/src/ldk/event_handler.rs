use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, bail, Context, Result};

use bitcoin::blockdata::locktime::absolute::LockTime;
use bitcoin::secp256k1::Secp256k1;

use crate::bitcoind::bitcoind_interface::BitcoindInterface;
use crate::database::forward::Forward;
use crate::database::payment::Payment;
use crate::database::{LdkDatabase, WalletDatabase};
use crate::ldk::peer_manager::KuutamoPeerManger;
use crate::log_error;
use crate::settings::Settings;
use lightning::chain::chaininterface::{BroadcasterInterface, ConfirmationTarget, FeeEstimator};
use lightning::events::{Event, PathFailure, PaymentPurpose};
use lightning::ln::ChannelId;
use lightning::routing::gossip::NodeId;
use lightning::sign::{KeysManager, SpendableOutputDescriptor};
use log::{error, info, trace, warn};
use rand::{thread_rng, Rng};
use tokio::runtime::Handle;

use crate::bitcoind::BitcoindClient;
use crate::ldk::{htlc_destination_to_string, ldk_error};
use crate::wallet::{Wallet, WalletInterface};

use super::controller::AsyncAPIRequests;
use super::peer_manager::PeerManager;
use super::{ChannelManager, KuutamoCustomMessageHandler, NetworkGraph};

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
    kuutamo_handler: Arc<KuutamoCustomMessageHandler>,
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
        kuutamo_handler: Arc<KuutamoCustomMessageHandler>,
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
            kuutamo_handler,
        }
    }
}

impl EventHandler {
    pub async fn handle_event_async(&self, event: lightning::events::Event) -> Result<()> {
        match event {
            Event::FundingGenerationReady {
                temporary_channel_id,
                counterparty_node_id,
                channel_value_satoshis,
                output_script,
                user_channel_id,
            } => {
                let (fee_rate, respond) = self
                    .async_api_requests
                    .funding_transactions
                    .get(&(user_channel_id as u64))
                    .await
                    .context(format!(
                        "Can't find funding transaction for user_channel_id {user_channel_id}"
                    ))?;

                let funding_tx =
                    match self
                        .wallet
                        .fund_tx(&output_script, &channel_value_satoshis, fee_rate)
                    {
                        Ok(tx) => tx,
                        Err(e) => {
                            respond(Err(anyhow!("Failed funding transaction: {e}")));
                            return Err(anyhow!("Failed funding transaction: {e}"));
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
                    respond(Err(anyhow!("Failed opening channel: {e}")));
                    bail!(e);
                }
                info!("EVENT: Channel with user channel id {user_channel_id} has been funded");
                if let Err(e) = self
                    .ldk_database
                    .update_initializing_channel(
                        &temporary_channel_id,
                        None,
                        Some(format!(
                            "Channel with user channel id {user_channel_id} has been funded"
                        )),
                    )
                    .await
                {
                    warn!("Fail to update initial channel funded status: {e}");
                }
                respond(Ok(funding_tx));
            }
            Event::ChannelPending {
                channel_id,
                user_channel_id,
                former_temporary_channel_id,
                counterparty_node_id,
                funding_txo,
            } => {
                info!(
                    "EVENT: Channel {} - {user_channel_id} with counterparty {counterparty_node_id} is pending. OutPoint: {funding_txo}",
                    hex::encode(channel_id.0),
                );
                if let Some(former_temporary_channel_id) = former_temporary_channel_id {
                    self.ldk_database
                        .update_initializing_channel(
                            &former_temporary_channel_id,
                            Some((&channel_id, funding_txo.vout)),
                            None::<&str>,
                        )
                        .await?;
                }

                // Insert ChannelDetails if we can list, else just create a channel record.
                if let Some(detail) = self
                    .channel_manager
                    .list_channels()
                    .iter()
                    .find(|c| c.channel_id == channel_id)
                {
                    self.ldk_database.persist_channel(detail).await?;
                } else {
                    self.ldk_database
                        .create_channel(&channel_id, true, &counterparty_node_id)
                        .await?;
                }
            }
            Event::ChannelReady {
                channel_id,
                user_channel_id,
                counterparty_node_id,
                channel_type: _,
            } => {
                // JIT Channels
                if user_channel_id > 9223372036854775808 {
                    if let Err(e) = self
                        .kuutamo_handler
                        .liquidity_manager
                        .lsps2_service_handler()
                        .expect("lsps2 handler should be set")
                        .channel_ready(user_channel_id, &channel_id, &counterparty_node_id)
                    {
                        error!("JIT Channel ready fail: {e:?}");
                    }
                }

                // LSPS2ServiceHandler::channel_ready
                info!(
                    "EVENT: Channel {} - {user_channel_id} with counterparty {counterparty_node_id} is ready to use.",
                    hex::encode(channel_id.0),
                );
                if let Some(channel_details) = self
                    .channel_manager
                    .list_channels()
                    .iter()
                    .find(|c| c.channel_id == channel_id)
                {
                    self.ldk_database.persist_channel(channel_details).await?;
                } else {
                    self.ldk_database
                        .close_channel(
                            &channel_id,
                            "Channel detail missing when receiving ChannelReady",
                        )
                        .await?;
                }
                info!("Broadcasting node announcement message");
                self.peer_manager
                    .broadcast_node_announcement_from_settings(self.settings.clone());
            }
            Event::ChannelClosed {
                channel_id,
                reason,
                user_channel_id,
                ..
            } => {
                info!("EVENT: Channel {}: {reason}.", hex::encode(channel_id.0));
                self.async_api_requests
                    .funding_transactions
                    .respond(
                        &(user_channel_id as u64),
                        Err(anyhow!("Channel closed due to {reason}")),
                    )
                    .await;
                self.ldk_database
                    .close_channel(&channel_id, format!("{reason}"))
                    .await?;
            }
            Event::DiscardFunding {
                channel_id,
                transaction,
            } => {
                info!(
                    "EVENT: Funding discarded for channel: {}, txid: {}",
                    hex::encode(channel_id.0),
                    transaction.txid()
                );
                if let Err(e) = self
                    .ldk_database
                    .close_channel(
                        &channel_id,
                        format!(
                            "Funding discarded for channel, txid: {}",
                            transaction.txid()
                        ),
                    )
                    .await
                {
                    error!("Fail to close channel which funding is discarded: {e}");
                }
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
                    hex::encode(payment_hash.0),
                    amount_msat,
                    if let Some(channel_id) = via_channel_id {
                        format!("via channel ID {} ", hex::encode(channel_id.0))
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
                ..
            } => {
                info!(
                    "EVENT: Payment claimed with hash {} of {} millisats",
                    hex::encode(payment_hash.0),
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
                self.ldk_database
                    .persist_payment(&payment)
                    .await
                    .context("Failed to persist payment")?;
            }
            Event::PaymentSent {
                payment_id,
                payment_preimage,
                payment_hash,
                fee_paid_msat,
            } => {
                info!(
                    "EVENT: Payment with hash {}{} sent successfully{}",
                    hex::encode(payment_hash.0),
                    if let Some(id) = payment_id {
                        format!(" and ID {}", hex::encode(id.0))
                    } else {
                        "".to_string()
                    },
                    if let Some(fee) = fee_paid_msat {
                        format!(" with fee {fee} msat")
                    } else {
                        "".to_string()
                    },
                );
                let payment_id = payment_id.context(format!(
                    "Failed to update payment with hash {}",
                    hex::encode(payment_hash.0)
                ))?;
                let (mut payment, respond) = self
                    .async_api_requests
                    .payments
                    .get(&payment_id)
                    .await
                    .context(format!(
                        "Can't find payment for {}",
                        hex::encode(payment_id.0)
                    ))?;
                payment.succeeded(payment_hash, payment_preimage, fee_paid_msat);
                respond(Ok(payment));
            }
            Event::PaymentPathSuccessful {
                payment_id,
                payment_hash,
                path,
            } => {
                info!(
                    "EVENT: Payment path with {} hops successful for payment with ID {}{}",
                    path.hops.len(),
                    hex::encode(payment_id.0),
                    payment_hash
                        .map(|h| format!(" and hash {}", hex::encode(h.0)))
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
                ..
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
                    hex::encode(payment_hash.0),
                    payment_id.map(|id| format!(" and ID {}", hex::encode(id.0))).unwrap_or_default(),
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
                    hex::encode(payment_id.0),
                    hex::encode(payment_hash.0),
                    reason
                        .map(|r| format!(" for reason {r:?}"))
                        .unwrap_or_default()
                );
                let (mut payment, respond) = self
                    .async_api_requests
                    .payments
                    .get(&payment_id)
                    .await
                    .context(format!(
                        "Can't find payment for {}",
                        hex::encode(payment_id.0)
                    ))?;
                payment.failed(reason);
                respond(Ok(payment));
            }
            Event::PaymentForwarded {
                prev_channel_id,
                next_channel_id,
                fee_earned_msat,
                claim_from_onchain_tx,
                outbound_amount_forwarded_msat,
            } => {
                if let Some(next_channel_id) = next_channel_id {
                    if let Err(e) = self
                        .kuutamo_handler
                        .liquidity_manager
                        .lsps2_service_handler()
                        .expect("lsps2 handler should be set")
                        .payment_forwarded(next_channel_id)
                    {
                        trace!("LSPS2 payment forward fail: {e:?}");
                    }
                }

                let read_only_network_graph = self.network_graph.read_only();
                let nodes = read_only_network_graph.nodes();
                let channels = self.channel_manager.list_channels();

                let node_str = |channel_id: &Option<ChannelId>| match channel_id {
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
                let channel_str = |channel_id: &Option<ChannelId>| {
                    channel_id
                        .map(|channel_id| format!(" with channel {}", hex::encode(channel_id.0)))
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
                let id = if let (
                    Some(inbound_channel_id),
                    Some(outbound_channel_id),
                    Some(amount),
                    Some(fee),
                ) = (
                    prev_channel_id,
                    next_channel_id,
                    outbound_amount_forwarded_msat,
                    fee_earned_msat,
                ) {
                    let forward =
                        Forward::success(inbound_channel_id, outbound_channel_id, amount, fee);
                    let id = forward.id.to_string();
                    self.persist_forward(forward);
                    format!(" with ID {id}")
                } else {
                    "".to_string()
                };
                info!(
                    "EVENT: Forwarded payment{id}{from_prev_str}{to_next_str} {amount_str},{fee_str} {from_onchain_str}",
                );
            }
            Event::ProbeSuccessful { .. } => {}
            Event::ProbeFailed { .. } => {}
            Event::HTLCHandlingFailed {
                prev_channel_id,
                failed_next_destination,
            } => {
                if let Err(e) = self
                    .kuutamo_handler
                    .liquidity_manager
                    .lsps2_service_handler()
                    .expect("lsps2 handler should be set")
                    .htlc_handling_failed(failed_next_destination.clone())
                {
                    trace!("LSPS2 htlc handling fail: {e:?}");
                };
                let forward = Forward::failure(prev_channel_id, failed_next_destination.clone());
                let id = forward.id.to_string();
                self.persist_forward(forward);
                error!(
                    "EVENT: Failed handling HTLC with ID {id} from channel {}. {}",
                    hex::encode(prev_channel_id.0),
                    htlc_destination_to_string(&failed_next_destination)
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
            Event::SpendableOutputs {
                outputs,
                channel_id,
            } => {
                for spendable_output in outputs.iter() {
                    info!("EVENT: New {:?}", spendable_output);
                    self.persist_spendable_output(spendable_output, channel_id.as_ref(), false)
                        .await;
                }
                let destination_address = self.wallet.new_internal_address()?;
                let tx_feerate = self
                    .bitcoind_client
                    .get_est_sat_per_1000_weight(ConfirmationTarget::OnChainSweep);

                let best_block_height = self.bitcoind_client.block_height().await?;
                let spending_tx = self
                    .keys_manager
                    .spend_spendable_outputs(
                        &outputs.iter().collect::<Vec<_>>()[..],
                        Vec::new(),
                        destination_address.script_pubkey(),
                        tx_feerate,
                        Some(LockTime::from_height(best_block_height as u32)?),
                        &Secp256k1::new(),
                    )
                    .map_err(|()| anyhow!("Failed to build spending transaction"))?;
                info!(
                    "Sending spendable output to {}",
                    destination_address.address
                );
                self.bitcoind_client.broadcast_transactions(&[&spending_tx]);
                for spendable_output in outputs.iter() {
                    self.persist_spendable_output(spendable_output, channel_id.as_ref(), true)
                        .await;
                }
            }
            Event::HTLCIntercepted {
                intercept_id,
                requested_next_hop_scid,
                payment_hash,
                inbound_amount_msat: _,
                expected_outbound_amount_msat,
            } => {
                if let Err(e) = self
                    .kuutamo_handler
                    .liquidity_manager
                    .lsps2_service_handler()
                    .expect("lsps2 handler should be set")
                    .htlc_intercepted(
                        requested_next_hop_scid,
                        intercept_id,
                        expected_outbound_amount_msat,
                        payment_hash,
                    )
                {
                    error!("HTLC Intercept fail: {e:?}");
                }
            }
            Event::InvoiceRequestFailed { payment_id } => {
                // XXX Handle it
                warn!("Invoice request failed, payment id: {payment_id:}");
            }
            Event::BumpTransaction(_) => unreachable!(),
            Event::ConnectionNeeded { node_id, .. } => {
                // XXX Handle it
                warn!("Need to connect to node {node_id:} for onion message");
            }
        };
        Ok(())
    }

    async fn persist_spendable_output(
        &self,
        spendable_output: &SpendableOutputDescriptor,
        channel_id: Option<&ChannelId>,
        is_spent: bool,
    ) {
        if let Err(e) = self
            .ldk_database
            .persist_spendable_output(spendable_output, channel_id, is_spent)
            .await
        {
            log_error(&e)
        }
    }

    fn persist_forward(&self, forward: Forward) {
        let database = self.ldk_database.clone();
        self.runtime_handle.spawn(async move {
            if let Err(e) = database.persist_forward(forward).await {
                log_error(&e)
            }
        });
    }
}
