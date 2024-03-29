use crate::database::{microsecond_timestamp, to_primitive, RowExt};
use crate::ldk::{ldk_error, ChainMonitor};
use crate::logger::KldLogger;
use crate::settings::Settings;
use bitcoin_hashes::Hash;

use super::forward::{Forward, ForwardStatus, TotalForwards};
use super::invoice::Invoice;
use super::payment::{Payment, PaymentDirection};
use super::{DurableConnection, Params};
use anyhow::bail;
use anyhow::{anyhow, Result};
use bitcoin::secp256k1::PublicKey;
use bitcoin::BlockHash;
use bitcoin::Txid;
use lightning::chain::chaininterface::{BroadcasterInterface, FeeEstimator};
use lightning::chain::chainmonitor::MonitorUpdateId;
use lightning::chain::channelmonitor::{ChannelMonitor, ChannelMonitorUpdate};
use lightning::chain::transaction::OutPoint;
use lightning::chain::{self, ChannelMonitorUpdateStatus, Watch};
use lightning::ln::channelmanager::{ChannelDetails, ChannelManager, ChannelManagerReadArgs};
use lightning::ln::msgs::SocketAddress;
use lightning::ln::ChannelId;
use lightning::ln::PaymentHash;
use lightning::routing::gossip::{NetworkGraph, NodeId};
use lightning::routing::router::Router;
use lightning::routing::scoring::{
    ProbabilisticScorer, ProbabilisticScoringDecayParameters, WriteableScore,
};
use lightning::sign::{
    ecdsa::WriteableEcdsaChannelSigner, EntropySource, NodeSigner, SignerProvider,
    SpendableOutputDescriptor,
};
use lightning::util::logger::Logger;
use lightning::util::persist::Persister;
use lightning::util::ser::ReadableArgs;
use lightning::util::ser::Writeable;
use log::{debug, error};

use super::peer::Peer;
use super::{ChannelRecord, SpendableOutputRecord};
use std::collections::HashMap;
use std::convert::{AsRef, TryInto};
use std::io::Cursor;
use std::ops::Deref;
use std::sync::{Arc, OnceLock};
use std::time::SystemTime;
use std::{fs, io};
use tokio::runtime::Handle;

pub struct LdkDatabase {
    settings: Arc<Settings>,
    durable_connection: Arc<DurableConnection>,
    // Persist graph/scorer gets called from a background thread in LDK so need a handle to the runtime.
    runtime: Handle,
    chain_monitor: OnceLock<Arc<ChainMonitor>>,
}

impl LdkDatabase {
    pub fn new(settings: Arc<Settings>, durable_connection: Arc<DurableConnection>) -> LdkDatabase {
        LdkDatabase {
            settings,
            durable_connection,
            runtime: Handle::current(),
            chain_monitor: OnceLock::new(),
        }
    }

    pub fn set_chain_monitor(&self, chain_monitor: Arc<ChainMonitor>) {
        self.chain_monitor
            .set(chain_monitor)
            .map_err(|_| ())
            .expect("Incorrect initialisation");
    }

    pub async fn is_first_start(&self) -> Result<bool> {
        Ok(self
            .durable_connection
            .get()
            .await
            .query_opt("SELECT true FROM channel_manager", &[])
            .await?
            .is_none())
    }

    pub async fn persist_peer(&self, peer: &Peer) -> Result<()> {
        self.durable_connection
            .get()
            .await
            .execute(
                "UPSERT INTO peers (public_key, address) \
            VALUES ($1, $2)",
                &[&peer.public_key.encode(), &peer.address.encode()],
            )
            .await?;
        Ok(())
    }

    pub async fn fetch_peer(&self, public_key: &PublicKey) -> Result<Option<Peer>> {
        debug!("Fetching peer from database");
        self.durable_connection
            .get()
            .await
            .query_opt(
                "SELECT * FROM peers WHERE public_key = $1",
                &[&public_key.encode()],
            )
            .await?
            .map(|row| {
                let public_key: Vec<u8> = row.get("public_key");
                let net_address: Vec<u8> = row.get("address");
                Peer::deserialize(public_key, net_address)
            })
            .transpose()
    }

    pub async fn fetch_peers(&self) -> Result<HashMap<PublicKey, SocketAddress>> {
        debug!("Fetching peers from database");
        let mut peers = HashMap::new();
        for row in self
            .durable_connection
            .get()
            .await
            .query("SELECT * FROM peers", &[])
            .await?
        {
            let public_key: Vec<u8> = row.get("public_key");
            let address: Vec<u8> = row.get("address");
            let peer = Peer::deserialize(public_key, address)?;
            peers.insert(peer.public_key, peer.address);
        }
        debug!("Fetched {} peers", peers.len());
        Ok(peers)
    }

    pub async fn delete_peer(&self, public_key: &PublicKey) -> Result<()> {
        debug!("Delete peer");
        self.durable_connection
            .get()
            .await
            .execute(
                "DELETE FROM peers \
            WHERE public_key = $1",
                &[&public_key.encode()],
            )
            .await?;
        Ok(())
    }

    pub async fn persist_initializing_channel(
        &self,
        initializing_channel_id: &ChannelId,
        is_public: bool,
        counterparty: &PublicKey,
        txid: &Txid,
    ) -> Result<()> {
        debug!(
            "Initial record for initial channel {}",
            hex::encode(initializing_channel_id.0),
        );
        // let initializing_channel_id: &[u8; 32] = initializing_channel_id.0.as_ref();
        self.durable_connection
            .get()
            .await
            .execute(
                "INSERT INTO initializing_channels (
                    initializing_channel_id,
                    counterparty,
                    is_public,
                    txid
                ) VALUES ( $1, $2, $3, $4 )",
                &[
                    &initializing_channel_id.0.to_vec(),
                    &counterparty.encode(),
                    &is_public,
                    &txid.encode(),
                ],
            )
            .await?;
        Ok(())
    }

    pub async fn update_initializing_channel(
        &self,
        initializing_channel_id: &ChannelId,
        channel_id_with_vout: Option<(&ChannelId, u32)>,
        status: Option<impl AsRef<str>>,
    ) -> Result<()> {
        debug!(
            "Update record for initial channel {}",
            hex::encode(initializing_channel_id.0),
        );
        if let Some((channel_id, vout)) = channel_id_with_vout {
            let status = if let Some(status) = status {
                status.as_ref().to_string()
            } else {
                "Updated by ChannelPending Event".to_string()
            };
            self.durable_connection
                .get()
                .await
                .execute(
                    "UPDATE initializing_channels SET channel_id = $1, vout = $2, update_timestamp = $3, status = $4 WHERE initializing_channel_id= $5",
                    &[
                        &channel_id.0.to_vec(),
                        &(vout as i32),
                        &to_primitive(&microsecond_timestamp()),
                        &status.as_bytes(),
                        &initializing_channel_id.0.to_vec(),
                    ],
                )
                .await?;
        } else if let Some(status) = status {
            self.durable_connection
                .get()
                .await
                .execute(
                    "UPDATE initializing_channels SET status = $1, update_timestamp = $2 WHERE initializing_channel_id= $3",
                    &[
                        &status.as_ref().as_bytes(),
                        &to_primitive(&microsecond_timestamp()),
                        &initializing_channel_id.0.to_vec(),
                    ],
                )
                .await?;
        } else {
            error!(
                "Update initial channel {} with nothing",
                hex::encode(initializing_channel_id.0),
            );
        }
        Ok(())
    }

    /// Create a record for channel which is not usable and without channel detail
    pub async fn create_channel(
        &self,
        channel_id: &ChannelId,
        is_public: bool,
        counterparty: &PublicKey,
    ) -> Result<()> {
        debug!(
            "Create record for channel {} without detail",
            hex::encode(channel_id.0),
        );
        self.durable_connection
            .get()
            .await
            .execute(
                "INSERT INTO channels (
                    channel_id,
                    counterparty,
                    is_usable,
                    is_public
                ) VALUES ( $1, $2, false, $3 )",
                &[&channel_id.0.to_vec(), &counterparty.encode(), &is_public],
            )
            .await?;
        Ok(())
    }

    pub async fn persist_channel(&self, channel: &ChannelDetails) -> Result<()> {
        debug!("Persist channel {}", channel.channel_id);
        if let Some(scid) = &channel.short_channel_id {
            self.durable_connection
                .get()
                .await
                .execute(
                    "UPSERT INTO channels (
                        channel_id,
                        counterparty,
                        short_channel_id,
                        is_usable,
                        is_public,
                        data,
                        update_timestamp
                    ) VALUES ( $1, $2, $3, $4, $5, $6, $7)",
                    &[
                        &channel.channel_id.0.to_vec(),
                        &NodeId::from_pubkey(&channel.counterparty.node_id).encode(),
                        &(*scid as i64),
                        &channel.is_usable,
                        &channel.is_public,
                        &channel.encode(),
                        &to_primitive(&microsecond_timestamp()),
                    ],
                )
                .await?;
        } else {
            self.durable_connection
                .get()
                .await
                .execute(
                    "UPSERT INTO channels (
                        channel_id,
                        counterparty,
                        is_usable,
                        is_public,
                        data,
                        update_timestamp
                    ) VALUES ( $1, $2, $3, $4, $5, $6 )",
                    &[
                        &channel.channel_id.0.to_vec(),
                        &NodeId::from_pubkey(&channel.counterparty.node_id).encode(),
                        &channel.is_usable,
                        &channel.is_public,
                        &channel.encode(),
                        &to_primitive(&microsecond_timestamp()),
                    ],
                )
                .await?;
        }
        Ok(())
    }

    pub async fn close_channel(
        &self,
        channel_id: &ChannelId,
        closure_reason: impl AsRef<str>,
    ) -> Result<()> {
        debug!("Close channel {}", hex::encode(channel_id.0));
        self.durable_connection
            .get()
            .await
            .execute(
                "UPDATE channels SET is_usable = false, update_timestamp = $1, closure_reason = $2 WHERE channel_id = $3",
                &[
                    &to_primitive(&microsecond_timestamp()),
                    &closure_reason.as_ref().as_bytes(),
                    &channel_id.0.to_vec(),
                ],
            )
            .await?;
        Ok(())
    }

    pub async fn fetch_channel_history(&self) -> Result<Vec<ChannelRecord>> {
        let rows = self
            .durable_connection
            .get()
            .await
            .query(
                "SELECT
                    data,
                    open_timestamp,
                    update_timestamp,
                    closure_reason
            FROM
                channels
            WHERE is_usable = false",
                &[],
            )
            .await?;

        let mut outputs = vec![];
        for row in rows {
            let detail: Option<ChannelDetails> = row.read_optional("data")?;
            if let Some(mut detail) = detail {
                detail.is_usable = false;
                outputs.push(ChannelRecord {
                    channel_id: detail.channel_id.to_string(),
                    counterparty: detail.counterparty.node_id.to_string(),
                    open_timestamp: row.get_timestamp("open_timestamp"),
                    update_timestamp: row.get_timestamp("update_timestamp"),
                    closure_reason: row
                        .get::<&str, Option<&[u8]>>("closure_reason")
                        .map(|b| String::from_utf8_lossy(b).to_string()),
                    detail: Some(detail),
                });
            }
        }
        Ok(outputs)
    }

    pub async fn fetch_channels(&self) -> Result<Vec<ChannelRecord>> {
        let rows = self
            .durable_connection
            .get()
            .await
            .query(
                "SELECT
                    channel_id,
                    counterparty,
                    data,
                    is_usable,
                    open_timestamp,
                    update_timestamp,
                    closure_reason
                FROM
                    channels",
                &[],
            )
            .await?;

        let mut outputs = vec![];
        for row in rows {
            let mut detail: Option<ChannelDetails> = row.read_optional("data")?;
            if let Some(ref mut detail) = detail {
                detail.is_usable = row.get::<&str, bool>("is_usable");
            }
            let counterparty: PublicKey = row.read("counterparty")?;
            outputs.push(ChannelRecord {
                channel_id: ChannelId::from_bytes(row.read("channel_id")?).to_string(),
                counterparty: counterparty.to_string(),
                open_timestamp: row.get_timestamp("open_timestamp"),
                update_timestamp: row.get_timestamp("update_timestamp"),
                closure_reason: row
                    .get::<&str, Option<&[u8]>>("closure_reason")
                    .map(|b| String::from_utf8_lossy(b).to_string()),
                detail,
            });
        }
        Ok(outputs)
    }

    pub async fn persist_spendable_output(
        &self,
        descriptor: &SpendableOutputDescriptor,
        channel_id: Option<&ChannelId>,
        is_spent: bool,
    ) -> Result<()> {
        let (txid, index, value) = match descriptor {
            SpendableOutputDescriptor::StaticOutput {
                outpoint, output, ..
            } => (outpoint.txid, outpoint.index, output.value),
            SpendableOutputDescriptor::DelayedPaymentOutput(descriptor) => (
                descriptor.outpoint.txid,
                descriptor.outpoint.index,
                descriptor.output.value,
            ),
            SpendableOutputDescriptor::StaticPaymentOutput(descriptor) => (
                descriptor.outpoint.txid,
                descriptor.outpoint.index,
                descriptor.output.value,
            ),
        };
        debug!("Persist spendable output {}:{}", txid, index);
        let mut data = vec![];
        descriptor.write(&mut data)?;

        let txid: &[u8] = txid.as_ref();
        if let Some(channel_id) = channel_id {
            self.durable_connection
                .get()
                .await
                .execute(
                    r#"UPSERT INTO spendable_outputs (
                        txid,
                        "index",
                        value,
                        channel_id,
                        data,
                        is_spent
                    ) VALUES ($1, $2, $3, $4, $5, $6)"#,
                    &[
                        &txid,
                        &(index as i16),
                        &(value as i64),
                        &channel_id.0.to_vec(),
                        &data,
                        &is_spent,
                    ],
                )
                .await?;
        } else {
            self.durable_connection
                .get()
                .await
                .execute(
                    r#"UPSERT INTO spendable_outputs (
                        txid,
                        "index",
                        value,
                        data,
                        is_spent
                    ) VALUES ($1, $2, $3, $4, $5)"#,
                    &[&txid, &(index as i16), &(value as i64), &data, &is_spent],
                )
                .await?;
        }
        Ok(())
    }

    pub async fn fetch_spendable_outputs(&self) -> Result<Vec<SpendableOutputRecord>> {
        let rows = self
            .durable_connection
            .get()
            .await
            .query(
                r#"SELECT
                data,
                is_spent
            FROM
                spendable_outputs"#,
                &[],
            )
            .await?;

        let mut outputs = vec![];
        for row in rows {
            outputs.push(SpendableOutputRecord {
                descriptor: row.read("data")?,
                is_spent: row.get::<&str, bool>("is_spent"),
            });
        }
        Ok(outputs)
    }

    pub async fn persist_invoice(&self, invoice: &Invoice) -> Result<()> {
        debug!(
            "Persist invoice with hash: {}",
            hex::encode(invoice.payment_hash.0)
        );

        let payment_hash: &[u8] = invoice.payment_hash.0.as_ref();
        self.durable_connection
            .get()
            .await
            .execute(
                "UPSERT INTO invoices (
                    payment_hash,
                    label,
                    bolt11,
                    payee_pub_key,
                    expiry,
                    amount,
                    timestamp
                ) VALUES ($1, $2, $3, $4, $5, $6, $7)",
                &[
                    &payment_hash,
                    &invoice.label,
                    &invoice.bolt11.to_string(),
                    &invoice.payee_pub_key.encode(),
                    &(invoice.bolt11.expiry_time().as_secs() as i64),
                    &invoice.amount.map(|a| a as i64),
                    &invoice.timestamp,
                ],
            )
            .await?;
        Ok(())
    }

    pub async fn fetch_invoices(&self, label: Option<String>) -> Result<Vec<Invoice>> {
        debug!("Fetching invoices from database");
        let connection = self.durable_connection.get().await;
        let mut params = Params::default();
        let mut query = "
            SELECT
                i.label as invoice_label,
                i.payment_hash,
                i.bolt11,
                i.expiry,
                i.amount as invoice_amount,
                i.payee_pub_key,
                i.timestamp as invoice_timestamp,
                p.id,
                p.hash,
                p.preimage,
                p.secret,
                p.status,
                p.amount,
                p.fee,
                p.direction,
                p.timestamp,
                p.label
            FROM invoices i
            LEFT OUTER JOIN payments p ON i.payment_hash = p.hash"
            .to_string();
        if let Some(label) = &label {
            params.push(label);
            query.push_str(&format!("\nWHERE i.label = ${}", params.count()));
        }
        let mut invoices: HashMap<PaymentHash, Invoice> = HashMap::new();
        for row in connection.query(&query, &params.to_params()).await? {
            let payment_hash: Vec<u8> = row.get("payment_hash");
            let payment_hash = PaymentHash(payment_hash.as_slice().try_into()?);
            let payment = if row.try_get::<&str, PaymentDirection>("direction").is_ok() {
                Some(Payment::try_from(&row)?)
            } else {
                None
            };
            if let Some(invoice) = invoices.get_mut(&payment_hash) {
                if let Some(payment) = payment {
                    invoice.payments.push(payment);
                }
            } else {
                let label: Option<String> = row.get("invoice_label");
                let bolt11: String = row.get("bolt11");
                let expiry: Option<i64> = row.get("expiry");
                let payee_pub_key: Vec<u8> = row.get("payee_pub_key");
                let amount: Option<i64> = row.get("invoice_amount");
                let timestamp: SystemTime = row.get("invoice_timestamp");
                let mut invoice = Invoice::deserialize(
                    payment_hash,
                    label,
                    bolt11,
                    payee_pub_key,
                    expiry.map(|i| i as u64),
                    amount,
                    timestamp,
                )?;
                if let Some(payment) = payment {
                    invoice.payments.push(payment);
                }
                invoices.insert(invoice.payment_hash, invoice);
            }
        }
        Ok(Vec::from_iter(invoices.into_values()))
    }

    pub async fn persist_payment(&self, payment: &Payment) -> Result<()> {
        debug!("Persist payment id: {}", hex::encode(payment.id.0));
        self.durable_connection
            .get()
            .await
            .execute(
                "UPSERT INTO payments (
                    id,
                    hash,
                    preimage,
                    secret,
                    label,
                    status,
                    amount,
                    fee,
                    direction,
                    timestamp
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                &[
                    &payment.id.0.to_vec(),
                    &payment.hash.as_ref().map(|x| x.0.to_vec()),
                    &payment.preimage.as_ref().map(|x| x.0.to_vec()),
                    &payment.secret.as_ref().map(|s| s.0.to_vec()),
                    &payment.label,
                    &payment.status,
                    &(payment.amount as i64),
                    &payment.fee.map(|f| f as i64).as_ref(),
                    &payment.direction,
                    &to_primitive(&payment.timestamp),
                ],
            )
            .await?;
        Ok(())
    }

    pub async fn fetch_payments(
        &self,
        payment_hash: Option<PaymentHash>,
        direction: Option<PaymentDirection>,
    ) -> Result<Vec<Payment>> {
        let connection = self.durable_connection.get().await;
        let mut payments = vec![];
        let mut params = Params::default();
        let mut query = "
            SELECT
                p.id,
                p.hash,
                p.preimage,
                p.secret,
                p.label,
                p.status,
                p.amount,
                p.fee,
                p.direction,
                p.timestamp,
                i.bolt11
            FROM payments as p
            LEFT OUTER JOIN invoices i ON p.hash = i.payment_hash
            WHERE 1 = 1"
            .to_string();
        if let Some(hash) = &payment_hash {
            params.push(hash.0.to_vec());
            query.push_str(&format!("AND p.hash = ${}", params.count()));
        }
        if let Some(direction) = direction {
            params.push(direction);
            query.push_str(&format!("AND p.direction = ${}", params.count()));
        }
        for row in connection
            .query(&query.to_string(), &params.to_params())
            .await?
        {
            payments.push(Payment::try_from(&row)?);
        }
        Ok(payments)
    }

    pub async fn persist_forward(&self, forward: Forward) -> Result<()> {
        debug!("Persist forward with ID {}", forward.id);

        let htlc_destination = if let Some(htlc_destination) = forward.htlc_destination {
            let mut bytes = vec![];
            htlc_destination.write(&mut bytes)?;
            Some(bytes)
        } else {
            None
        };
        self.durable_connection
            .get()
            .await
            .execute(
                "UPSERT INTO forwards (
                    id,
                    inbound_channel_id,
                    outbound_channel_id,
                    amount,
                    fee,
                    status,
                    htlc_destination,
                    timestamp
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                &[
                    &forward.id,
                    &forward.inbound_channel_id.0.to_vec(),
                    &forward.outbound_channel_id.as_ref().map(|x| x.0.to_vec()),
                    &(forward.amount.map(|x| x as i64)),
                    &(forward.fee.map(|x| x as i64)),
                    &forward.status,
                    &htlc_destination,
                    &to_primitive(&forward.timestamp),
                ],
            )
            .await?;
        Ok(())
    }

    pub async fn fetch_forwards(&self, status: Option<ForwardStatus>) -> Result<Vec<Forward>> {
        let mut statement = "
            SELECT
                id,
                inbound_channel_id,
                outbound_channel_id,
                amount,
                fee,
                status,
                htlc_destination,
                timestamp
            FROM
                forwards
            "
        .to_string();
        let mut params = Params::default();
        if let Some(status) = status {
            statement.push_str("WHERE status = $1");
            params.push(status);
        }
        statement.push_str("ORDER BY timestamp ASC");
        let mut forwards = vec![];
        let rows = self
            .durable_connection
            .get()
            .await
            .query(&statement, &params.to_params())
            .await?;

        for row in rows {
            forwards.push(row.try_into()?);
        }
        Ok(forwards)
    }

    pub async fn fetch_total_forwards(&self) -> Result<TotalForwards> {
        Ok(self
            .durable_connection
            .get()
            .await
            .query_one(
                "SELECT
                    count(*) AS count,
                    COALESCE(CAST(sum(amount) AS INT), 0) AS amount,
                    COALESCE(CAST(sum(fee) AS INT), 0) AS fee
                FROM forwards
                WHERE status = 'succeeded';",
                &[],
            )
            .await?
            .into())
    }

    pub async fn fetch_channel_monitors<T: EntropySource + SignerProvider>(
        &self,
        source: &T,
    ) -> Result<
        Vec<(
            BlockHash,
            ChannelMonitor<<T as SignerProvider>::EcdsaSigner>,
        )>,
    > {
        let rows = self
            .durable_connection
            .wait()
            .await
            .query(
                "SELECT out_point, monitor \
            FROM channel_monitors",
                &[],
            )
            .await?;
        let mut monitors: Vec<(
            BlockHash,
            ChannelMonitor<<T as SignerProvider>::EcdsaSigner>,
        )> = vec![];
        for row in rows {
            let out_point: Vec<u8> = row.get("out_point");

            let (txid_bytes, index_bytes) = out_point.split_at(32);
            let txid = Txid::from_raw_hash(bitcoin_hashes::sha256d::Hash::from_slice(txid_bytes)?);
            let index = u16::from_be_bytes(index_bytes.try_into().unwrap());

            let monitor: Vec<u8> = row.get("monitor");
            let mut buffer = Cursor::new(&monitor);
            match <(
                BlockHash,
                ChannelMonitor<<T as SignerProvider>::EcdsaSigner>,
            )>::read(&mut buffer, (source, source))
            {
                Ok((blockhash, channel_monitor)) => {
                    if channel_monitor.get_funding_txo().0.txid != txid
                        || channel_monitor.get_funding_txo().0.index != index
                    {
                        bail!("Unable to find ChannelMonitor for: {}:{}", txid, index);
                    }
                    monitors.push((blockhash, channel_monitor));
                }
                Err(e) => bail!("Failed to deserialize ChannelMonitor: {}", e),
            }
        }
        Ok(monitors)
    }

    pub async fn fetch_channel_manager<
        M: Deref,
        T: Deref,
        ES: Deref,
        NS: Deref,
        SP: Deref,
        F: Deref,
        R: Deref,
        L: Deref,
    >(
        &self,
        read_args: ChannelManagerReadArgs<'_, M, T, ES, NS, SP, F, R, L>,
    ) -> Result<(BlockHash, ChannelManager<M, T, ES, NS, SP, F, R, L>)>
    where
        <M as Deref>::Target: Watch<<SP::Target as SignerProvider>::EcdsaSigner>,
        <T as Deref>::Target: BroadcasterInterface,
        <ES as Deref>::Target: EntropySource,
        <NS as Deref>::Target: NodeSigner,
        <SP as Deref>::Target: SignerProvider,
        <F as Deref>::Target: FeeEstimator,
        <R as Deref>::Target: Router,
        <L as Deref>::Target: Logger,
    {
        let row = self
            .durable_connection
            .get()
            .await
            .query_one(
                "SELECT manager \
            FROM channel_manager",
                &[],
            )
            .await?;
        let manager: Vec<u8> = row.get("manager");
        <(BlockHash, ChannelManager<M, T, ES, NS, SP, F, R, L>)>::read(
            &mut Cursor::new(manager),
            read_args,
        )
        .map_err(|e| anyhow!(e.to_string()))
    }

    pub async fn fetch_graph(&self) -> Result<Option<NetworkGraph<Arc<KldLogger>>>> {
        match fs::read(format!("{}/network_graph", self.settings.data_dir)) {
            Ok(bytes) => {
                let graph = NetworkGraph::read(&mut Cursor::new(bytes), KldLogger::global())
                    .map_err(|e| anyhow!(e))?;
                Ok(Some(graph))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(anyhow!(e)),
        }
    }

    pub async fn fetch_scorer(
        &self,
        params: ProbabilisticScoringDecayParameters,
        graph: Arc<NetworkGraph<Arc<KldLogger>>>,
    ) -> Result<
        Option<(
            ProbabilisticScorer<Arc<NetworkGraph<Arc<KldLogger>>>, Arc<KldLogger>>,
            SystemTime,
        )>,
    > {
        let scorer = self
            .durable_connection
            .wait()
            .await
            .query_opt("SELECT scorer, timestamp FROM scorer", &[])
            .await?
            .map(|row| {
                let bytes: Vec<u8> = row.get(0);
                let timestamp: SystemTime = row.get(1);
                let scorer = ProbabilisticScorer::read(
                    &mut Cursor::new(bytes),
                    (params, graph.clone(), KldLogger::global()),
                )
                .expect("Unable to deserialize scorer");
                (scorer, timestamp)
            });
        Ok(scorer)
    }

    pub async fn fetch_scorer_binary(&self) -> Result<Vec<u8>> {
        let row = self
            .durable_connection
            .wait()
            .await
            .query_one("SELECT scorer FROM scorer;", &[])
            .await?;
        Ok(row.get("scorer"))
    }
}

impl<'a, M: Deref, T: Deref, ES: Deref, NS: Deref, SP: Deref, F: Deref, R: Deref, L: Deref, S>
    Persister<'a, M, T, ES, NS, SP, F, R, L, S> for LdkDatabase
where
    M::Target: 'static + Watch<<SP::Target as SignerProvider>::EcdsaSigner>,
    T::Target: 'static + BroadcasterInterface,
    ES::Target: 'static + EntropySource,
    NS::Target: 'static + NodeSigner,
    SP::Target: 'static + SignerProvider,
    F::Target: 'static + FeeEstimator,
    R::Target: 'static + Router,
    L::Target: 'static + Logger,
    S: 'static + WriteableScore<'a>,
{
    fn persist_manager(
        &self,
        channel_manager: &ChannelManager<M, T, ES, NS, SP, F, R, L>,
    ) -> Result<(), io::Error> {
        let mut buf = vec![];
        channel_manager.write(&mut buf)?;
        let durable_connection = self.durable_connection.clone();
        self.runtime.spawn_blocking(async move || {
            if let Err(e) = durable_connection
                .get()
                .await
                .execute(
                    "UPSERT INTO channel_manager (id, manager, timestamp) \
                        VALUES ('manager', $1, CURRENT_TIMESTAMP)",
                    &[&buf],
                )
                .await
            {
                error!("Failed to persist channel manager: {e}");
            }
        });
        Ok(())
    }

    // Network graph could get very large so just write it to disk for now.
    fn persist_graph(
        &self,
        network_graph: &lightning::routing::gossip::NetworkGraph<L>,
    ) -> Result<(), io::Error> {
        let mut buf = vec![];
        network_graph.write(&mut buf)?;
        if let Err(e) = fs::write(format!("{}/network_graph", self.settings.data_dir), &buf) {
            error!("Failed to persist graph: {e}");
        }
        Ok(())
    }

    fn persist_scorer(&self, scorer: &S) -> Result<(), io::Error> {
        let mut buf = vec![];
        scorer.write(&mut buf)?;
        let durable_connection = self.durable_connection.clone();
        self.runtime.spawn(async move {
            if let Err(e) = durable_connection
                .get()
                .await
                .execute(
                    "UPSERT INTO scorer (id, scorer, timestamp)
                        VALUES ('scorer', $1, CURRENT_TIMESTAMP)",
                    &[&buf],
                )
                .await
            {
                error!("Failed to persist scorer: {e}");
            }
        });
        Ok(())
    }
}

impl<ChannelSigner: WriteableEcdsaChannelSigner> chain::chainmonitor::Persist<ChannelSigner>
    for LdkDatabase
{
    // The CHANNEL_MONITORS table stores the latest monitor and its update_id.
    fn persist_new_channel(
        &self,
        funding_txo: OutPoint,
        monitor: &ChannelMonitor<ChannelSigner>,
        update_id: MonitorUpdateId,
    ) -> ChannelMonitorUpdateStatus {
        debug!(
            "Persisting channel: {:?} {}",
            funding_txo,
            monitor.get_latest_update_id()
        );
        let mut out_point_buf = vec![];
        funding_txo.write(&mut out_point_buf).unwrap();

        let mut monitor_buf = vec![];
        monitor.write(&mut monitor_buf).unwrap();
        let latest_update_id = monitor.get_latest_update_id();

        let durable_connection = self.durable_connection.clone();
        let chain_monitor = self
            .chain_monitor
            .get()
            .expect("bad initialisation")
            .clone();
        tokio::spawn(async move {
            let result = durable_connection
                .get()
                .await
                .execute(
                    "UPSERT INTO channel_monitors (out_point, monitor, update_id) \
                VALUES ($1, $2, $3)",
                    &[&out_point_buf, &monitor_buf, &(latest_update_id as i64)],
                )
                .await;
            match result {
                Ok(_) => {
                    debug!(
                        "Stored channel: {}:{} with update id: {}",
                        funding_txo.txid, funding_txo.index, latest_update_id
                    );
                    if let Err(e) = chain_monitor.channel_monitor_updated(funding_txo, update_id) {
                        error!("Failed to update chain monitor: {}", ldk_error(e));
                    }
                }
                Err(e) => {
                    error!("Failed to persist channel update: {e}");
                }
            }
        });
        ChannelMonitorUpdateStatus::InProgress
    }

    // Updates are applied to the monitor when fetched from database.
    fn update_persisted_channel(
        &self,
        funding_txo: OutPoint,
        _update: Option<&ChannelMonitorUpdate>,
        monitor: &ChannelMonitor<ChannelSigner>,
        update_id: MonitorUpdateId,
    ) -> ChannelMonitorUpdateStatus {
        self.persist_new_channel(funding_txo, monitor, update_id)

        // Hope we can enable this soon. Probably after https://github.com/lightningdevkit/rust-lightning/issues/1426
        /*
                let mut out_point_buf = vec![];
                funding_txo.write(&mut out_point_buf).unwrap();

                // If its the last update then store the last monitor and delete the updates.
                if update.as_ref().map_or(true, |x| x.update_id == CLOSED_CHANNEL_UPDATE_ID) {
                    let mut monitor_buf = vec![];
                    monitor.write(&mut monitor_buf).unwrap();
                    let ciphertext = self.cipher.encrypt(&monitor_buf);

                    tokio::task::block_in_place(move || {
                        Handle::current().block_on(async move {
                            let mut lock = self.client.write().await;
                            let tx = lock.transaction().await.unwrap();
                            tx.execute(
                                "UPSERT INTO channel_monitors (out_point, monitor, update_id) VALUES ($1, $2, $3)",
                                &[
                                    &out_point_buf,
                                    &ciphertext,
                                    &(monitor.get_latest_update_id() as i64),
                                ],
                            )
                            .await
                            .unwrap();
                            let deleted = tx
                                .execute(
                                    "DELETE FROM channel_monitor_updates WHERE out_point = $1",
                                    &[&out_point_buf],
                                )
                                .await
                                .unwrap();
                            tx.commit().await.unwrap();
                            debug!("Stored latest monitor and deleted {} updates.", deleted);
                        })
                    })
                } else {
                    let update = update.as_ref().unwrap();
                    let mut update_buf = vec![];
                    update.write(&mut update_buf).unwrap();
                    let ciphertext = self.cipher.encrypt(&update_buf);

                    block_in_place!(
                        "UPSERT INTO channel_monitor_updates (out_point, update, update_id) \
                        VALUES ($1, $2, $3)",
                        &[&out_point_buf, &ciphertext, &(update.update_id as i64)],
                        self
                    );
                }
                ChannelMonitorUpdateStatus::Completed
        */
    }
}
