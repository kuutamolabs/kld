use crate::ldk::ChainMonitor;
use crate::logger::KldLogger;
use crate::to_i64;

use super::invoice::Invoice;
use super::payment::Payment;
use super::DurableConnection;
use anyhow::{anyhow, bail, Context, Result};
use bitcoin::hashes::hex::ToHex;
use bitcoin::hashes::Hash;
use bitcoin::secp256k1::PublicKey;
use bitcoin::{BlockHash, Txid};
use lightning::chain::chaininterface::{BroadcasterInterface, FeeEstimator};
use lightning::chain::chainmonitor::MonitorUpdateId;
use lightning::chain::channelmonitor::{ChannelMonitor, ChannelMonitorUpdate};
use lightning::chain::keysinterface::{
    EntropySource, NodeSigner, SignerProvider, WriteableEcdsaChannelSigner,
};
use lightning::chain::transaction::OutPoint;
use lightning::chain::{self, ChannelMonitorUpdateStatus, Watch};
use lightning::ln::channelmanager::{ChannelManager, ChannelManagerReadArgs, PaymentId};
use lightning::ln::msgs::NetAddress;
use lightning::ln::{PaymentHash, PaymentPreimage, PaymentSecret};
use lightning::routing::gossip::NetworkGraph;
use lightning::routing::router::Router;
use lightning::routing::scoring::{
    ProbabilisticScorer, ProbabilisticScoringParameters, WriteableScore,
};
use lightning::util::logger::Logger;
use lightning::util::persist::Persister;
use lightning::util::ser::ReadableArgs;
use lightning::util::ser::Writeable;
use log::{debug, error, info};
use once_cell::sync::OnceCell;

use settings::Settings;
use std::collections::HashMap;
use std::convert::TryInto;
use std::io::Cursor;
use std::ops::Deref;
use std::sync::Arc;
use std::time::SystemTime;
use std::{fs, io};
use tokio::runtime::Handle;
use tokio_postgres::Row;

use super::peer::Peer;

pub struct LdkDatabase {
    settings: Arc<Settings>,
    durable_connection: Arc<DurableConnection>,
    // Persist graph/scorer gets called from a background thread in LDK so need a handle to the runtime.
    runtime: Handle,
    chain_monitor: OnceCell<Arc<ChainMonitor>>,
}

impl LdkDatabase {
    pub fn new(settings: Arc<Settings>, durable_connection: Arc<DurableConnection>) -> LdkDatabase {
        LdkDatabase {
            settings,
            durable_connection,
            runtime: Handle::current(),
            chain_monitor: OnceCell::new(),
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
                &[&peer.public_key.encode(), &peer.net_address.encode()],
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

    pub async fn fetch_peers(&self) -> Result<HashMap<PublicKey, NetAddress>> {
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
            let net_address: Vec<u8> = row.get("address");
            let peer = Peer::deserialize(public_key, net_address)?;
            peers.insert(peer.public_key, peer.net_address);
        }
        debug!("Fetched {} peers", peers.len());
        Ok(peers)
    }

    pub async fn delete_peer(&self, public_key: &PublicKey) -> Result<()> {
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

    pub async fn persist_invoice(&self, invoice: &Invoice) -> Result<()> {
        debug!(
            "Persist invoice with hash: {}",
            invoice.payment_hash.0.to_hex()
        );
        self.durable_connection
            .get()
            .await
            .execute(
                "UPSERT INTO invoices (payment_hash, label, bolt11, payee_pub_key, expiry, amount) VALUES ($1, $2, $3, $4, $5, $6)",
                &[&invoice.payment_hash.0.as_ref(), &invoice.label, &invoice.bolt11.to_string(), &invoice.payee_pub_key.encode(), &(invoice.bolt11.expiry_time().as_secs() as i64), &invoice.amount.map(|a| a.as_i64())],
            )
            .await?;
        Ok(())
    }

    pub async fn fetch_invoices(&self, label: Option<String>) -> Result<Vec<Invoice>> {
        let mut query = "SELECT i.label as invoice_label, payment_hash, bolt11, expiry, i.amount as invoice_amount, payee_pub_key, id, hash, preimage, secret, status, p.amount as amount, fee, direction, p.timestamp as timestamp, p.label as label FROM invoices i LEFT OUTER JOIN payments p ON i.payment_hash = p.hash".to_string();
        let rows = if let Some(label) = label {
            query.push_str(" WHERE i.label = $1");
            self.durable_connection
                .get()
                .await
                .query(&query, &[&label])
                .await?
        } else {
            self.durable_connection
                .get()
                .await
                .query(&query, &[])
                .await?
        };

        let mut invoices: HashMap<PaymentHash, Invoice> = HashMap::new();
        for row in rows {
            let payment_hash: Vec<u8> = row.get("payment_hash");
            let payment_hash = PaymentHash(payment_hash.as_slice().try_into()?);
            let payment = if row.try_get::<&str, Vec<u8>>("id").is_ok() {
                Some(parse_payment(&row)?)
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
                let mut invoice = Invoice::deserialize(
                    payment_hash,
                    label,
                    bolt11,
                    payee_pub_key,
                    expiry.map(|i| i as u64),
                    amount,
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
        debug!("Persist payment: {}", payment.hash.0.to_hex());
        self.durable_connection
            .get()
            .await
            .execute(
                "UPSERT INTO payments (id, hash, preimage, secret, label, status, amount, fee, direction, timestamp) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                &[&payment.id.0.as_ref(), &payment.hash.0.as_ref(), &payment.preimage.as_ref().map(|x| x.0.as_ref()), &payment.secret.as_ref().map(|s| s.0.as_ref()), &payment.label, &payment.status, &payment.amount.as_i64(), &payment.fee.as_ref().map(|f| f.as_i64()), &payment.direction, &payment.timestamp],
            )
            .await?;
        Ok(())
    }

    pub async fn fetch_payments(&self) -> Result<Vec<Payment>> {
        let mut payments = vec![];
        let rows = self
            .durable_connection
            .get()
            .await
            .query("SELECT * FROM payments", &[])
            .await?;
        for row in rows {
            let id: &[u8] = row.get("id");
            let hash: &[u8] = row.get("hash");
            let preimage: Option<&[u8]> = row.get("preimage");
            let secret: Option<&[u8]> = row.get("secret");

            let preimage = match preimage {
                Some(bytes) => Some(PaymentPreimage(bytes.try_into().context("bad preimage")?)),
                None => None,
            };
            let secret = match secret {
                Some(bytes) => Some(PaymentSecret(bytes.try_into().context("bad secret")?)),
                None => None,
            };

            payments.push(Payment {
                id: PaymentId(id.try_into().context("bad ID")?),
                hash: PaymentHash(hash.try_into().context("bad hash")?),
                preimage,
                secret,
                label: row.get("label"),
                status: row.get("status"),
                amount: row.get::<&str, i64>("amount").into(),
                fee: row.get::<&str, Option<i64>>("fee").map(|f| f.into()),
                direction: row.get("direction"),
                timestamp: row.get("timestamp"),
            })
        }
        Ok(payments)
    }

    pub async fn fetch_channel_monitors<ES: EntropySource, SP: SignerProvider>(
        &self,
        entropy_source: &ES,
        signer_provider: &SP, //		broadcaster: &B,
                              //		fee_estimator: &F,
    ) -> Result<Vec<(BlockHash, ChannelMonitor<SP::Signer>)>>
where
        //      B::Target: BroadcasterInterface,
        //		F::Target: FeeEstimator,
    {
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
        let mut monitors: Vec<(BlockHash, ChannelMonitor<SP::Signer>)> = vec![];
        for row in rows {
            let out_point: Vec<u8> = row.get("out_point");

            let (txid_bytes, index_bytes) = out_point.split_at(32);
            let txid = Txid::from_slice(txid_bytes).unwrap();
            let index = u16::from_be_bytes(index_bytes.try_into().unwrap());

            let monitor: Vec<u8> = row.get("monitor");
            let mut buffer = Cursor::new(&monitor);
            match <(BlockHash, ChannelMonitor<SP::Signer>)>::read(
                &mut buffer,
                (entropy_source, signer_provider),
            ) {
                Ok((blockhash, channel_monitor)) => {
                    if channel_monitor.get_funding_txo().0.txid != txid
                        || channel_monitor.get_funding_txo().0.index != index
                    {
                        bail!("Unable to find ChannelMonitor for: {}:{}", txid, index);
                    }
                    /*
                                        let update_rows = self
                                            .client
                                            .read()
                                            .await
                                            .query(
                                                "SELECT update \
                                            FROM channel_monitor_updates \
                                            WHERE out_point = $1 \
                                            ORDER BY update_id ASC",
                                                &[&out_point],
                                            )
                                            .await
                                            .unwrap();

                                        let updates: Vec<ChannelMonitorUpdate> = update_rows
                                            .iter()
                                            .map(|row| {
                                                let ciphertext: Vec<u8> = row.get("update");
                                                let update = self.cipher.decrypt(&ciphertext);
                                                ChannelMonitorUpdate::read(&mut Cursor::new(&update)).unwrap()
                                            })
                                            .collect();
                                        for update in updates {
                                            channel_monitor
                                                .update_monitor(&update, broadcaster, fee_estimator.clone(), &KndLogger::global()).unwrap();
                                        }
                    */
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
        <M as Deref>::Target: Watch<<SP::Target as SignerProvider>::Signer>,
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
        params: ProbabilisticScoringParameters,
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
                    (params.clone(), graph.clone(), KldLogger::global()),
                )
                .expect("Unable to deserialize scorer");
                (scorer, timestamp)
            });
        Ok(scorer)
    }
}

fn parse_payment(row: &Row) -> Result<Payment> {
    let id: &[u8] = row.get("id");
    let hash: &[u8] = row.get("hash");
    let preimage: Option<&[u8]> = row.get("preimage");
    let secret: Option<&[u8]> = row.get("secret");
    let label: Option<String> = row.get("label");

    let preimage = match preimage {
        Some(bytes) => Some(PaymentPreimage(bytes.try_into().context("bad preimage")?)),
        None => None,
    };
    let secret = match secret {
        Some(bytes) => Some(PaymentSecret(bytes.try_into().context("bad secret")?)),
        None => None,
    };

    Ok(Payment {
        id: PaymentId(id.try_into().context("bad ID")?),
        hash: PaymentHash(hash.try_into().context("bad hash")?),
        preimage,
        secret,
        label,
        status: row.get("status"),
        amount: row.get::<&str, i64>("amount").into(),
        fee: row.get::<&str, Option<i64>>("fee").map(|f| f.into()),
        direction: row.get("direction"),
        timestamp: row.get("timestamp"),
    })
}

impl<'a, M: Deref, T: Deref, ES: Deref, NS: Deref, SP: Deref, F: Deref, R: Deref, L: Deref, S>
    Persister<'a, M, T, ES, NS, SP, F, R, L, S> for LdkDatabase
where
    M::Target: 'static + Watch<<SP::Target as SignerProvider>::Signer>,
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
        self.runtime.spawn(async move {
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
        _update_id: MonitorUpdateId,
    ) -> ChannelMonitorUpdateStatus {
        let mut out_point_buf = vec![];
        funding_txo.write(&mut out_point_buf).unwrap();

        let mut monitor_buf = vec![];
        monitor.write(&mut monitor_buf).unwrap();
        let latest_update_id = monitor.get_latest_update_id();

        let durable_connection = self.durable_connection.clone();
        // Storing the updates async makes things way more complicated. So even though its a little slower we stick with sync for now.
        tokio::task::block_in_place(move || {
            tokio::runtime::Handle::current().block_on(async move {
                let result = durable_connection
                    .get()
                    .await
                    .execute(
                        "UPSERT INTO channel_monitors (out_point, monitor, update_id) \
                VALUES ($1, $2, $3)",
                        &[&out_point_buf, &monitor_buf, &to_i64!(latest_update_id)],
                    )
                    .await;
                match result {
                    Ok(_) => {
                        info!(
                            "Stored channel: {}:{} with update id: {}",
                            funding_txo.txid, funding_txo.index, latest_update_id
                        );
                        ChannelMonitorUpdateStatus::Completed
                    }
                    Err(e) => {
                        error!("Failed to persist channel update: {e}");
                        ChannelMonitorUpdateStatus::PermanentFailure
                    }
                }
            })
        })
    }

    // Updates are applied to the monitor when fetched from database.
    fn update_persisted_channel(
        &self,
        funding_txo: OutPoint,
        _update: Option<&ChannelMonitorUpdate>,
        monitor: &ChannelMonitor<ChannelSigner>,
        update_id: MonitorUpdateId,
    ) -> ChannelMonitorUpdateStatus {
        debug!(
            "Updating persisted channel: {:?}:{}",
            funding_txo,
            monitor.get_latest_update_id()
        );
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
                                    &to_i64!(monitor.get_latest_update_id()),
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
                        &[&out_point_buf, &ciphertext, &to_i64!(update.update_id)],
                        self
                    );
                }
                ChannelMonitorUpdateStatus::Completed
        */
    }
}
