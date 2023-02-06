use crate::{connection, to_i64, Client};
use anyhow::{anyhow, bail, Result};
use bitcoin::hashes::Hash;
use bitcoin::secp256k1::PublicKey;
use bitcoin::{BlockHash, Txid};
use lightning::chain::chaininterface::{BroadcasterInterface, FeeEstimator};
use lightning::chain::chainmonitor::MonitorUpdateId;
use lightning::chain::channelmonitor::{ChannelMonitor, ChannelMonitorUpdate};
use lightning::chain::keysinterface::{KeysInterface, Sign};
use lightning::chain::transaction::OutPoint;
use lightning::chain::{self, ChannelMonitorUpdateStatus, Watch};
use lightning::ln::channelmanager::{ChannelManager, ChannelManagerReadArgs};
use lightning::routing::gossip::NetworkGraph;
use lightning::routing::scoring::{
    ProbabilisticScorer, ProbabilisticScoringParameters, WriteableScore,
};
use lightning::util::logger::Logger;
use lightning::util::persist::Persister;
use lightning::util::ser::ReadableArgs;
use lightning::util::ser::Writeable;
use log::{debug, info};
use logger::KndLogger;
use settings::Settings;
use std::convert::TryInto;
use std::io::Cursor;
use std::ops::Deref;
use std::sync::Arc;
use tokio::runtime::Handle;
use tokio::sync::RwLock;

use crate::peer::Peer;

// This gets called from a background thread in LDK so need a handle to the runtime.
macro_rules! block_in_place {
    ($statement: literal, $params: expr, $self: expr) => {
        tokio::task::block_in_place(move || {
            $self.runtime.block_on(async move {
                $self
                    .client()
                    .await
                    .unwrap()
                    .read()
                    .await
                    .execute($statement, $params)
                    .await
                    .unwrap()
            })
        })
    };
}

pub struct LdkDatabase {
    settings: Settings,
    client: Arc<RwLock<Client>>,
    runtime: Handle,
}

impl LdkDatabase {
    pub async fn new(settings: &Settings) -> Result<LdkDatabase> {
        info!(
            "Connecting LDK to Cockroach database {} at {}:{}",
            settings.database_name, settings.database_host, settings.database_port
        );
        let client = connection(settings).await?;
        let client = Arc::new(RwLock::new(client));

        Ok(LdkDatabase {
            settings: settings.clone(),
            client,
            runtime: Handle::current(),
        })
    }

    /// Try to reconnect to the database if the connection has been dropped.
    /// If this is not possible one of the callers of this function should shut the node down.
    async fn client(&self) -> Result<Arc<RwLock<Client>>> {
        if self.client.read().await.is_closed() {
            let mut guard = self.client.write().await;
            if guard.is_closed() {
                let client = connection(&self.settings).await?;
                *guard = client;
            }
        }
        Ok(self.client.clone())
    }

    pub async fn is_first_start(&self) -> Result<bool> {
        Ok(self
            .client()
            .await?
            .read()
            .await
            .query_opt("SELECT true FROM channel_manager", &[])
            .await?
            .is_none())
    }

    pub async fn persist_peer(&self, peer: &Peer) -> Result<()> {
        self.client()
            .await?
            .read()
            .await
            .execute(
                "UPSERT INTO peers (public_key, address) \
            VALUES ($1, $2)",
                &[
                    &peer.public_key.encode().as_slice(),
                    &peer.socket_addr.to_string().as_bytes(),
                ],
            )
            .await?;
        Ok(())
    }

    pub async fn fetch_peer(&self, public_key: &PublicKey) -> Result<Option<Peer>> {
        debug!("Fetching peer from database");
        let peer = self
            .client()
            .await?
            .read()
            .await
            .query_opt(
                "SELECT * FROM peers WHERE public_key = $1",
                &[&public_key.encode()],
            )
            .await?
            .map(|row| {
                let public_key: Vec<u8> = row.get("public_key");
                let address: Vec<u8> = row.get("address");
                Peer {
                    public_key: PublicKey::from_slice(&public_key).unwrap(),
                    socket_addr: String::from_utf8(address).unwrap().parse().unwrap(),
                }
            });
        Ok(peer)
    }

    pub async fn fetch_peers(&self) -> Result<Vec<Peer>> {
        debug!("Fetching peers from database");
        let mut peers = Vec::new();
        for row in self
            .client()
            .await?
            .read()
            .await
            .query("SELECT * FROM peers", &[])
            .await?
        {
            let public_key: Vec<u8> = row.get("public_key");
            let address: Vec<u8> = row.get("address");
            peers.push(Peer {
                public_key: PublicKey::from_slice(&public_key).unwrap(),
                socket_addr: String::from_utf8(address)?.parse().unwrap(),
            });
        }
        debug!("Fetched {} peers", peers.len());
        Ok(peers)
    }

    pub async fn delete_peer(&self, public_key: &PublicKey) -> Result<()> {
        self.client()
            .await?
            .read()
            .await
            .execute(
                "DELETE FROM peers \
            WHERE public_key = $1",
                &[&public_key.encode()],
            )
            .await?;
        Ok(())
    }

    pub async fn fetch_channel_monitors<Signer: Sign, K: Deref>(
        &self,
        keys_manager: K,
        //		broadcaster: &B,
        //		fee_estimator: &F,
    ) -> Result<Vec<(BlockHash, ChannelMonitor<Signer>)>>
    where
        <K as Deref>::Target: KeysInterface<Signer = Signer> + Sized,
        //      B::Target: BroadcasterInterface,
        //		F::Target: FeeEstimator,
    {
        let rows = self
            .client()
            .await?
            .read()
            .await
            .query(
                "SELECT out_point, monitor \
            FROM channel_monitors",
                &[],
            )
            .await?;
        let mut monitors: Vec<(BlockHash, ChannelMonitor<Signer>)> = vec![];
        for row in rows {
            let out_point: Vec<u8> = row.get("out_point");

            let (txid_bytes, index_bytes) = out_point.split_at(32);
            let txid = Txid::from_slice(txid_bytes).unwrap();
            let index = u16::from_le_bytes(index_bytes.try_into().unwrap());

            let monitor: Vec<u8> = row.get("monitor");
            let mut buffer = Cursor::new(&monitor);
            match <(BlockHash, ChannelMonitor<Signer>)>::read(&mut buffer, &*keys_manager) {
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
        Signer: Sign,
        M: Deref,
        T: Deref,
        K: Deref,
        F: Deref,
        L: Deref,
    >(
        &self,
        read_args: ChannelManagerReadArgs<'_, M, T, K, F, L>,
    ) -> Result<(BlockHash, ChannelManager<M, T, K, F, L>)>
    where
        <M as Deref>::Target: Watch<Signer>,
        <T as Deref>::Target: BroadcasterInterface,
        <K as Deref>::Target: KeysInterface<Signer = Signer>,
        <F as Deref>::Target: FeeEstimator,
        <L as Deref>::Target: Logger,
    {
        let row = self
            .client()
            .await?
            .read()
            .await
            .query_one(
                "SELECT manager \
            FROM channel_manager",
                &[],
            )
            .await?;
        let manager: Vec<u8> = row.get("manager");
        <(BlockHash, ChannelManager<M, T, K, F, L>)>::read(&mut Cursor::new(manager), read_args)
            .map_err(|e| anyhow!(e.to_string()))
    }

    pub async fn fetch_graph(&self) -> Result<Option<NetworkGraph<Arc<KndLogger>>>> {
        let graph = self
            .client()
            .await?
            .read()
            .await
            .query_opt("SELECT graph FROM network_graph", &[])
            .await?
            .map(|row| {
                let bytes: Vec<u8> = row.get(0);
                NetworkGraph::read(&mut Cursor::new(bytes), KndLogger::global())
                    .expect("Unable to deserialize network graph")
            });
        Ok(graph)
    }

    pub async fn fetch_scorer(
        &self,
        params: ProbabilisticScoringParameters,
        graph: Arc<NetworkGraph<Arc<KndLogger>>>,
    ) -> Result<Option<ProbabilisticScorer<Arc<NetworkGraph<Arc<KndLogger>>>, Arc<KndLogger>>>>
    {
        let scorer = self
            .client()
            .await?
            .read()
            .await
            .query_opt("SELECT scorer FROM scorer", &[])
            .await?
            .map(|row| {
                let bytes: Vec<u8> = row.get(0);
                ProbabilisticScorer::read(
                    &mut Cursor::new(bytes),
                    (params.clone(), graph.clone(), KndLogger::global()),
                )
                .expect("Unable to deserialize scorer")
            });
        Ok(scorer)
    }
}

impl<'a, Signer: Sign, M: Deref, T: Deref, K: Deref, F: Deref, L: Deref, S>
    Persister<'a, M, T, K, F, L, S> for LdkDatabase
where
    M::Target: 'static + chain::Watch<Signer>,
    T::Target: 'static + BroadcasterInterface,
    K::Target: 'static + KeysInterface<Signer = Signer>,
    F::Target: 'static + FeeEstimator,
    L::Target: 'static + Logger,
    S: 'static + WriteableScore<'a>,
{
    fn persist_manager(
        &self,
        channel_manager: &ChannelManager<M, T, K, F, L>,
    ) -> Result<(), std::io::Error> {
        let mut buf = vec![];
        channel_manager.write(&mut buf).unwrap();
        block_in_place!(
            "UPSERT INTO channel_manager (id, manager, timestamp) \
            VALUES ('manager', $1, CURRENT_TIMESTAMP)",
            &[&buf],
            self
        );
        Ok(())
    }

    fn persist_graph(
        &self,
        network_graph: &lightning::routing::gossip::NetworkGraph<L>,
    ) -> Result<(), std::io::Error> {
        let mut buf = vec![];
        network_graph.write(&mut buf).unwrap();
        block_in_place!(
            "UPSERT INTO network_graph (id, graph, timestamp)
            VALUES ('graph', $1, CURRENT_TIMESTAMP)",
            &[&buf],
            self
        );
        Ok(())
    }

    fn persist_scorer(&self, scorer: &S) -> Result<(), std::io::Error> {
        let mut buf = vec![];
        scorer.write(&mut buf).unwrap();
        block_in_place!(
            "UPSERT INTO scorer (id, scorer, timestamp)
            VALUES ('scorer', $1, CURRENT_TIMESTAMP)",
            &[&buf],
            self
        );
        Ok(())
    }
}

impl<ChannelSigner: Sign> chain::chainmonitor::Persist<ChannelSigner> for LdkDatabase {
    // The CHANNEL_MONITORS table stores the latest monitor and its update_id.
    fn persist_new_channel(
        &self,
        funding_txo: OutPoint,
        monitor: &ChannelMonitor<ChannelSigner>,
        _update_id: MonitorUpdateId,
    ) -> ChannelMonitorUpdateStatus {
        debug!(
            "Persisting new channel: {:?}:{}",
            funding_txo,
            monitor.get_latest_update_id()
        );

        let mut out_point_buf = vec![];
        funding_txo.write(&mut out_point_buf).unwrap();

        let mut monitor_buf = vec![];
        monitor.write(&mut monitor_buf).unwrap();

        block_in_place!(
            "UPSERT INTO channel_monitors (out_point, monitor, update_id) \
            VALUES ($1, $2, $3)",
            &[
                &out_point_buf,
                &monitor_buf,
                &to_i64!(monitor.get_latest_update_id())
            ],
            self
        );
        ChannelMonitorUpdateStatus::Completed
    }

    // Updates are applied to the monitor when fetched from database.
    fn update_persisted_channel(
        &self,
        funding_txo: OutPoint,
        _update: &Option<ChannelMonitorUpdate>,
        monitor: &ChannelMonitor<ChannelSigner>,
        update_id: MonitorUpdateId, // only need this if persisting async.
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
