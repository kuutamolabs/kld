use std::{net::SocketAddr, sync::Arc, time::Duration};

use crate::api::SocketAddress;
use crate::bitcoind::{BitcoindClient, BitcoindUtxoLookup};
use crate::database::{peer::Peer, LdkDatabase};
use crate::logger::KldLogger;
use crate::settings::Settings;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use bitcoin::secp256k1::PublicKey;
use ldk_lsp_client::LiquidityManager;
use lightning::sign::KeysManager;
use lightning::{
    chain::Filter,
    ln::{channelmanager::SimpleArcChannelManager, peer_handler},
    onion_message::SimpleArcOnionMessenger,
    routing::gossip,
};
use lightning_net_tokio::SocketDescriptor;
use log::{error, info, warn};
use tokio::task::JoinHandle;

use super::{ChainMonitor, ChannelManager, KldRouter};

pub(crate) type PeerManager = peer_handler::PeerManager<
    SocketDescriptor,
    Arc<SimpleArcChannelManager<ChainMonitor, BitcoindClient, BitcoindClient, KldLogger>>,
    Arc<
        gossip::P2PGossipSync<
            Arc<gossip::NetworkGraph<Arc<KldLogger>>>,
            Arc<BitcoindUtxoLookup>,
            Arc<KldLogger>,
        >,
    >,
    Arc<SimpleArcOnionMessenger<KldLogger>>,
    Arc<KldLogger>,
    Arc<
        LiquidityManager<
            Arc<KeysManager>,
            Arc<ChainMonitor>,
            Arc<BitcoindClient>,
            Arc<BitcoindClient>,
            Arc<KldRouter>,
            Arc<KeysManager>,
            Arc<KldLogger>,
            Arc<KeysManager>,
            Arc<dyn Filter + Send + Sync>,
        >,
    >,
    Arc<KeysManager>,
>;

#[async_trait]
pub trait KuutamoPeerManger {
    async fn listen(&self, port: u16) -> Result<()>;
    async fn connect_peer(
        &self,
        database: Arc<LdkDatabase>,
        public_key: PublicKey,
        peer_addr: SocketAddress,
    ) -> Result<()>;

    fn keep_channel_peers_connected(
        &self,
        database: Arc<LdkDatabase>,
        channel_manager: Arc<ChannelManager>,
    );

    fn get_connected_peers(&self) -> Vec<(PublicKey, Option<SocketAddress>)>;

    fn is_connected(&self, public_key: &PublicKey) -> bool;

    async fn disconnect_and_drop_by_node_id(
        &self,
        database: Arc<LdkDatabase>,
        node_id: PublicKey,
    ) -> Result<()>;

    /// broadcast the node alias and public addresses of current setting
    fn broadcast_node_announcement_from_settings(&self, settings: Arc<Settings>);
}

#[async_trait]
impl KuutamoPeerManger for Arc<PeerManager> {
    async fn listen(&self, port: u16) -> Result<()> {
        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port:}"))
            .await
            .context("Failed to bind to listen port")?;
        let peer_manager = self.clone();
        tokio::spawn(async move {
            loop {
                let peer_mgr = peer_manager.clone();
                match listener.accept().await {
                    Ok((tcp_stream, socket_addr)) => {
                        if let Ok(tcp_stream) = tcp_stream.into_std() {
                            tokio::spawn(async move {
                                let disconnected = lightning_net_tokio::setup_inbound(
                                    peer_mgr.clone(),
                                    tcp_stream,
                                );
                                info!("Inbound peer connection from {socket_addr}");
                                disconnected.await;
                                info!("Inbound peer disconnected from {socket_addr}");
                            });
                        } else {
                            warn!("tokio tcp stream fail into standard stream")
                        }
                    }
                    Err(e) => warn!("fail to acept peer socket {e}"),
                }
            }
        });
        Ok(())
    }
    async fn connect_peer(
        &self,
        database: Arc<LdkDatabase>,
        public_key: PublicKey,
        peer_addr: SocketAddress,
    ) -> Result<()> {
        if self.is_connected(&public_key) {
            return Ok(());
        }
        let handle = connect_peer(self.clone(), database, public_key, peer_addr).await?;
        loop {
            if self.is_connected(&public_key) {
                return Ok(());
            }
            if handle.is_finished() {
                return Err(anyhow!("Peer disconnected"));
            }
            tokio::time::sleep(Duration::from_secs(1)).await
        }
    }
    fn keep_channel_peers_connected(
        &self,
        database: Arc<LdkDatabase>,
        channel_manager: Arc<ChannelManager>,
    ) {
        let peer_manager = self.clone();
        tokio::spawn(async move {
            loop {
                let connected_node_ids = peer_manager.get_peer_node_ids();
                for unconnected_node_id in channel_manager
                    .list_channels()
                    .iter()
                    .map(|chan| chan.counterparty.node_id)
                    .filter(|id| !connected_node_ids.iter().any(|(pk, _)| pk == id))
                {
                    match database.fetch_peer(&unconnected_node_id).await {
                        Ok(Some(peer)) => {
                            let _ = connect_peer(
                                peer_manager.clone(),
                                database.clone(),
                                peer.public_key,
                                peer.address.into(),
                            )
                            .await;
                        }
                        Err(e) => error!("{}", e),
                        _ => (),
                    }
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
    }

    fn get_connected_peers(&self) -> Vec<(PublicKey, Option<SocketAddress>)> {
        self.get_peer_node_ids()
            .into_iter()
            .map(|(k, a)| (k, a.map(SocketAddress::from)))
            .collect()
    }

    fn is_connected(&self, public_key: &PublicKey) -> bool {
        self.get_peer_node_ids().iter().any(|p| p.0 == *public_key)
    }

    async fn disconnect_and_drop_by_node_id(
        &self,
        database: Arc<LdkDatabase>,
        node_id: PublicKey,
    ) -> Result<()> {
        self.disconnect_by_node_id(node_id);
        database.delete_peer(&node_id).await
    }

    fn broadcast_node_announcement_from_settings(&self, settings: Arc<Settings>) {
        let mut alias = [0; 32];
        alias[..settings.node_alias.len()].copy_from_slice(settings.node_alias.as_bytes());
        let addresses: Vec<lightning::ln::msgs::SocketAddress> = settings
            .public_addresses
            .clone()
            .into_iter()
            .map(|a| a.inner())
            .collect();
        self.broadcast_node_announcement([0; 3], alias, addresses);
    }
}

async fn connect_peer(
    peer_manager: Arc<PeerManager>,
    database: Arc<LdkDatabase>,
    public_key: PublicKey,
    address: SocketAddress,
) -> Result<JoinHandle<()>> {
    let socket_addr = SocketAddr::try_from(address.clone())?;
    let connection_closed =
        lightning_net_tokio::connect_outbound(peer_manager, public_key, socket_addr)
            .await
            .context("Could not connect to peer {public_key}@{peer_addr}")?;
    database
        .persist_peer(&Peer {
            public_key,
            address: address.0,
        })
        .await?;
    info!("Connected to peer {public_key}@{socket_addr}");
    Ok(tokio::spawn(async move {
        connection_closed.await;
        info!("Disconnected from peer {public_key}@{socket_addr}");
    }))
}
