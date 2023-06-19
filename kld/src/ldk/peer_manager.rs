use std::{net::SocketAddr, sync::Arc, time::Duration};

use crate::database::{peer::Peer, LdkDatabase};
use anyhow::{anyhow, bail, Context, Result};
use api::NetAddress;
use bitcoin::secp256k1::PublicKey;
use log::{error, info};
use settings::Settings;
use tokio::task::JoinHandle;

use super::{ChannelManager, LdkPeerManager};

pub struct PeerManager {
    ldk_peer_manager: Arc<LdkPeerManager>,
    channel_manager: Arc<ChannelManager>,
    database: Arc<LdkDatabase>,
    settings: Arc<Settings>,
    addresses: Vec<NetAddress>,
}

impl PeerManager {
    pub fn new(
        ldk_peer_manager: Arc<LdkPeerManager>,
        channel_manager: Arc<ChannelManager>,
        database: Arc<LdkDatabase>,
        settings: Arc<Settings>,
    ) -> Result<PeerManager> {
        if settings.node_alias.len() > 32 {
            bail!("Node Alias can not be longer than 32 bytes");
        }
        let addresses = settings.public_addresses();
        Ok(PeerManager {
            ldk_peer_manager,
            channel_manager,
            database,
            settings,
            addresses,
        })
    }

    pub async fn listen(&self) {
        let listener =
            tokio::net::TcpListener::bind(format!("0.0.0.0:{}", self.settings.peer_port))
                .await
                .context("Failed to bind to listen port")
                .unwrap();
        let ldk_peer_manager = self.ldk_peer_manager.clone();
        tokio::spawn(async move {
            loop {
                let peer_mgr = ldk_peer_manager.clone();
                let (tcp_stream, socket_addr) = listener.accept().await.unwrap();
                tokio::spawn(async move {
                    let disconnected = lightning_net_tokio::setup_inbound(
                        peer_mgr.clone(),
                        tcp_stream.into_std().unwrap(),
                    );
                    info!("Inbound peer connection from {socket_addr}");
                    disconnected.await;
                    info!("Inbound peer disconnected from {socket_addr}");
                });
            }
        });
    }

    pub async fn connect_peer(&self, public_key: PublicKey, peer_addr: NetAddress) -> Result<()> {
        if self.is_connected(&public_key) {
            return Ok(());
        }
        let handle = connect_peer(
            self.ldk_peer_manager.clone(),
            self.database.clone(),
            public_key,
            peer_addr,
        )
        .await?;
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

    pub fn keep_channel_peers_connected(&self) {
        let database = self.database.clone();
        let ldk_peer_manager = self.ldk_peer_manager.clone();
        let channel_manager = self.channel_manager.clone();
        tokio::spawn(async move {
            loop {
                let connected_node_ids = ldk_peer_manager.get_peer_node_ids();
                for unconnected_node_id in channel_manager
                    .list_channels()
                    .iter()
                    .map(|chan| chan.counterparty.node_id)
                    .filter(|id| !connected_node_ids.iter().any(|(pk, _)| pk == id))
                {
                    match database.fetch_peer(&unconnected_node_id).await {
                        Ok(Some(peer)) => {
                            let _ = connect_peer(
                                ldk_peer_manager.clone(),
                                database.clone(),
                                peer.public_key,
                                peer.net_address.into(),
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

    // This is only be relayed by the gossip network if we have some public channels.
    pub fn broadcast_node_announcement(&self) {
        let mut alias = [0; 32];
        alias[..self.settings.node_alias.len()]
            .copy_from_slice(self.settings.node_alias.as_bytes());
        let peer_manager = self.ldk_peer_manager.clone();
        let addresses: Vec<api::lightning::ln::msgs::NetAddress> = self
            .addresses
            .clone()
            .into_iter()
            .map(|a| a.inner())
            .collect();
        peer_manager.broadcast_node_announcement([0; 3], alias, addresses);
    }

    pub fn get_connected_peers(&self) -> Vec<(PublicKey, Option<NetAddress>)> {
        self.ldk_peer_manager
            .get_peer_node_ids()
            .into_iter()
            .map(|(k, a)| (k, a.map(NetAddress::from)))
            .collect()
    }

    pub fn is_connected(&self, public_key: &PublicKey) -> bool {
        self.ldk_peer_manager
            .get_peer_node_ids()
            .iter()
            .any(|p| p.0 == *public_key)
    }

    pub async fn disconnect_by_node_id(&self, node_id: PublicKey) -> Result<()> {
        self.ldk_peer_manager.disconnect_by_node_id(node_id);
        self.database.delete_peer(&node_id).await
    }

    pub fn disconnect_all_peers(&self) {
        self.ldk_peer_manager.disconnect_all_peers();
    }
}

async fn connect_peer(
    ldk_peer_manager: Arc<LdkPeerManager>,
    database: Arc<LdkDatabase>,
    public_key: PublicKey,
    peer_address: NetAddress,
) -> Result<JoinHandle<()>> {
    let socket_addr = SocketAddr::try_from(peer_address.clone())?;
    let connection_closed =
        lightning_net_tokio::connect_outbound(ldk_peer_manager, public_key, socket_addr)
            .await
            .context("Could not connect to peer {public_key}@{peer_addr}")?;
    database
        .persist_peer(&Peer {
            public_key,
            net_address: peer_address.0,
        })
        .await?;
    info!("Connected to peer {public_key}@{socket_addr}");
    Ok(tokio::spawn(async move {
        connection_closed.await;
        info!("Disconnected from peer {public_key}@{socket_addr}");
    }))
}
