use std::{io::Cursor, ops::Deref, sync::Arc};

use crate::{
    cryptor::Cryptor,
    s3::{S3Bucket, S3},
};
use bitcoin::{
    hashes::hex::{FromHex, ToHex},
    BlockHash, Txid,
};
use lightning::{
    chain::{
        self,
        chaininterface::{BroadcasterInterface, FeeEstimator},
        chainmonitor::MonitorUpdateId,
        channelmonitor::{ChannelMonitor, ChannelMonitorUpdate},
        keysinterface::{KeysInterface, Sign},
        transaction::OutPoint,
        ChannelMonitorUpdateStatus, Watch,
    },
    ln::channelmanager::{ChannelManager, ChannelManagerReadArgs},
    routing::scoring::{ProbabilisticScorer, ProbabilisticScoringParameters, WriteableScore},
    util::{logger::Logger, persist::Persister, ser::ReadableArgs},
};
use lightning::{routing::gossip::NetworkGraph, util::ser::Writeable};
use logger::KndLogger;
use settings::Settings;
use tokio::runtime::Handle;

pub struct ObjectStorage {
    cryptor: Cryptor,
    s3: Arc<S3>,
}

impl ObjectStorage {
    pub async fn new(settings: &Settings) -> ObjectStorage {
        let cryptor = Cryptor::new(&settings);
        let minio = Arc::new(S3::new(&settings).await);
        ObjectStorage { cryptor, s3: minio }
    }

    pub async fn fetch_channel_monitors<Signer: Sign, K: Deref, F: Deref, T: Deref, L: Deref>(
        &self,
        keys_manager: K,
        _fee_estimator: F,
        _broadcaster: T,
        _logger: L,
    ) -> Result<Vec<(BlockHash, ChannelMonitor<Signer>)>, std::io::Error>
    where
        <T as Deref>::Target: BroadcasterInterface,
        <K as Deref>::Target: KeysInterface<Signer = Signer> + Sized,
        <F as Deref>::Target: FeeEstimator,
        <L as Deref>::Target: Logger,
    {
        let mut result = vec![];
        let monitor_paths = self.s3.list(S3Bucket::Keys, "monitors", None).await;
        for path in monitor_paths {
            let key = path.split_at(9).1.split_at(70).0;
            let txid = key.split_at(64).0;
            let txid = Txid::from_hex(txid).unwrap();
            let index = key.split_at(65).1;
            let index: u16 = index.parse().unwrap();
            let ciphertext = self.s3.get(S3Bucket::Keys, &path).await;
            let bytes = self.cryptor.decrypt(&ciphertext);

            let (blockhash, channel_monitor) = match <(BlockHash, ChannelMonitor<Signer>)>::read(
                &mut Cursor::new(bytes),
                &*keys_manager,
            ) {
                Ok((blockhash, channel_monitor)) => {
                    if channel_monitor.get_funding_txo().0.txid != txid
                        || channel_monitor.get_funding_txo().0.index != index
                    {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "ChannelMonitor was stored in the wrong file",
                        ));
                    }
                    (blockhash, channel_monitor)
                }
                Err(e) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Failed to deserialize ChannelMonitor: {}", e),
                    ))
                }
            };
            //let mut updates = self.minio.get_all(S3Bucket::Keys, &format!("updates/{}/", &key), None).await;
            //updates.sort_by_cached_key(|k| k.0.split_at(79).1.parse::<u32>().unwrap());
            //for update in updates {
            //	let bytes = self.cryptor.decrypt(&update.1);
            //	let cmu = ChannelMonitorUpdate::read(&mut Cursor::new(&bytes)).unwrap();
            //	if cmu.update_id > channel_monitor.get_latest_update_id() {
            //		channel_monitor.update_monitor(&cmu, &broadcaster, &fee_estimator, &logger).unwrap();
            //	}
            //}
            result.push((blockhash, channel_monitor));
        }
        Ok(result)
    }

    pub async fn manager_exists(&self) -> bool {
        self.s3.exists(S3Bucket::Keys, "manager").await
    }

    pub async fn read_manager<Signer: Sign, M: Deref, T: Deref, K: Deref, F: Deref, L: Deref>(
        &self,
        read_args: ChannelManagerReadArgs<'_, Signer, M, T, K, F, L>,
    ) -> Result<(BlockHash, ChannelManager<Signer, M, T, K, F, L>), String>
    where
        <M as Deref>::Target: Watch<Signer>,
        <T as Deref>::Target: BroadcasterInterface,
        <K as Deref>::Target: KeysInterface<Signer = Signer>,
        <F as Deref>::Target: FeeEstimator,
        <L as Deref>::Target: Logger,
    {
        let ciphertext = self.s3.get(S3Bucket::Keys, "manager").await;
        let bytes = self.cryptor.decrypt(&ciphertext);
        Ok(<(BlockHash, ChannelManager<Signer, M, T, K, F, L>)>::read(
            &mut Cursor::new(bytes),
            read_args,
        )
        .unwrap())
    }

    pub async fn read_graph(
        &self,
        genesis_hash: BlockHash,
        logger: Arc<KndLogger>,
    ) -> NetworkGraph<Arc<KndLogger>> {
        let bytes = self
            .s3
            .get(S3Bucket::Graph, &format!("network_graph"))
            .await;
        if let Ok(graph) = NetworkGraph::read(&mut Cursor::new(bytes), logger.clone()) {
            return graph;
        }
        NetworkGraph::new(genesis_hash, logger)
    }

    pub async fn read_scorer<L: Clone + Deref>(
        &self,
        params: ProbabilisticScoringParameters,
        graph: Arc<NetworkGraph<Arc<KndLogger>>>,
        logger: Arc<KndLogger>,
    ) -> ProbabilisticScorer<Arc<NetworkGraph<Arc<KndLogger>>>, Arc<KndLogger>> {
        if self.s3.exists(S3Bucket::Keys, "scorer").await {
            let bytes = self.s3.get(S3Bucket::Keys, "scorer").await;
            if let Ok(scorer) = ProbabilisticScorer::read(
                &mut Cursor::new(bytes),
                (params.clone(), graph.clone(), logger.clone()),
            ) {
                return scorer;
            }
        }
        ProbabilisticScorer::new(params, graph, logger)
    }

    pub async fn key_exists(&self) -> bool {
        self.s3.exists(S3Bucket::Keys, "key").await
    }

    pub async fn persist_key(&self, key: &[u8; 32]) {
        let ciphertext = self.cryptor.encrypt(key);
        self.s3.put(S3Bucket::Keys, "key", &ciphertext).await;
    }

    pub async fn read_key(&self) -> [u8; 32] {
        let ciphertext = self.s3.get(S3Bucket::Keys, "key").await;
        self.cryptor.decrypt(&ciphertext).try_into().unwrap()
    }

    pub async fn delete_all(&self) {
        for path in self.s3.list(S3Bucket::Keys, "/", None).await {
            self.s3.delete(S3Bucket::Keys, &path).await;
        }
    }
}

impl<ChannelSigner: Sign> chain::chainmonitor::Persist<ChannelSigner> for ObjectStorage {
    fn persist_new_channel(
        &self,
        funding_txo: OutPoint,
        monitor: &ChannelMonitor<ChannelSigner>,
        _update_id: MonitorUpdateId,
    ) -> ChannelMonitorUpdateStatus {
        let path = format!(
            "monitors/{}_{:0>5}/monitor",
            funding_txo.txid.to_hex(),
            funding_txo.index
        );
        let mut monitor_buf = vec![];
        monitor.write(&mut monitor_buf).unwrap();
        let ciphertext = self.cryptor.encrypt(&monitor_buf);
        let minio = self.s3.clone();

        tokio::task::block_in_place(move || {
            Handle::current().block_on(async move {
                minio.put(S3Bucket::Keys, &path, &ciphertext).await;
                ChannelMonitorUpdateStatus::Completed
            })
        })
    }

    // Updates are applied to the monitor when fetched from database.
    // TODO make this async using update_id when it is clear how to handle circular dependency ChainMonitor <-> Persister
    fn update_persisted_channel(
        &self,
        funding_txo: OutPoint,
        _update: &Option<ChannelMonitorUpdate>,
        monitor: &ChannelMonitor<ChannelSigner>,
        _update_id: MonitorUpdateId,
    ) -> ChannelMonitorUpdateStatus {
        let key = format!("{}_{:0>5}", funding_txo.txid.to_hex(), funding_txo.index);
        // If its the last update then store the last monitor and delete the updates.
        //if update.is_none() || update.as_ref().map_or(false, |x| x.update_id == CLOSED_CHANNEL_UPDATE_ID) {
        let mut monitor_buf = vec![];
        monitor.write(&mut monitor_buf).unwrap();
        let ciphertext = self.cryptor.encrypt(&monitor_buf);
        let minio = self.s3.clone();
        tokio::task::block_in_place(move || {
            Handle::current().block_on(async move {
                minio
                    .put(
                        S3Bucket::Keys,
                        &format!("monitors/{}/monitor", key),
                        &ciphertext,
                    )
                    .await;
                //			for update in minio.list(S3Bucket::Keys, &format!("updates/{}", key), None).await {
                //				minio.delete(S3Bucket::Keys, &update).await;
                //			}
                ChannelMonitorUpdateStatus::Completed
            })
        })

        //} else {
        //	let update = update.as_ref().unwrap();
        //	let path = format!("updates/{}/{}", key, update.update_id);
        //	let mut update_buf = vec![];
        //	update.write(&mut update_buf).unwrap();
        //	let ciphertext = self.cryptor.encrypt(&update_buf);
        //	let minio = self.minio.clone();
        //	tokio::task::block_in_place(move || {
        //		Handle::current().block_on(async move {
        //			minio.put(S3Bucket::Keys, &path, &ciphertext).await;
        //			Ok::<(), ()>(())
        //		})
        //	})
    }
}

impl<'a, Signer: Sign, M: Deref, T: Deref, K: Deref, F: Deref, L: Deref, S>
    Persister<'a, Signer, M, T, K, F, L, S> for ObjectStorage
where
    M::Target: 'static + chain::Watch<Signer>,
    T::Target: 'static + BroadcasterInterface,
    K::Target: 'static + KeysInterface<Signer = Signer>,
    F::Target: 'static + FeeEstimator,
    L::Target: 'static + Logger,
    S: WriteableScore<'a>,
{
    fn persist_manager(
        &self,
        channel_manager: &ChannelManager<Signer, M, T, K, F, L>,
    ) -> Result<(), std::io::Error> {
        let mut buf = vec![];
        channel_manager.write(&mut buf).unwrap();
        let ciphertext = self.cryptor.encrypt(&buf);
        self.s3.put_blocking(S3Bucket::Keys, "manager", &ciphertext);
        Ok(())
    }

    fn persist_graph(&self, network_graph: &NetworkGraph<L>) -> Result<(), std::io::Error> {
        let mut buf = vec![];
        network_graph.write(&mut buf).unwrap();
        self.s3.put_blocking(S3Bucket::Graph, "graph", &buf);
        Ok(())
    }

    fn persist_scorer(&self, scorer: &S) -> Result<(), std::io::Error> {
        let mut buf = vec![];
        scorer.write(&mut buf).unwrap();
        self.s3.put_blocking(S3Bucket::Graph, "scorer", &buf);
        Ok(())
    }
}
