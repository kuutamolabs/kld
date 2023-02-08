use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::{bail, Context, Result};
use bitcoin::secp256k1::PublicKey;
use database::ldk_database::LdkDatabase;
use log::{error, info};
use settings::Settings;

use crate::{
    controller::{ChannelManager, LdkPeerManager},
    net_utils,
};

pub struct PeerManager {
    ldk_peer_manager: Arc<LdkPeerManager>,
    channel_manager: Arc<ChannelManager>,
    database: Arc<LdkDatabase>,
    settings: Arc<Settings>,
}

impl PeerManager {
    pub fn new(
        ldk_peer_manager: Arc<LdkPeerManager>,
        channel_manager: Arc<ChannelManager>,
        database: Arc<LdkDatabase>,
        settings: Arc<Settings>,
    ) -> Result<PeerManager> {
        if settings.knd_node_name.len() > 32 {
            bail!("Node Alias can not be longer than 32 bytes");
        }
        Ok(PeerManager {
            ldk_peer_manager,
            channel_manager,
            database,
            settings,
        })
    }

    pub async fn listen(&self) -> Result<()> {
        let listener =
            tokio::net::TcpListener::bind(format!("0.0.0.0:{}", self.settings.knd_peer_port))
                .await
                .context("Failed to bind to listen port")?;
        let ldk_peer_manager = self.ldk_peer_manager.clone();
        tokio::spawn(async move {
            loop {
                let peer_mgr = ldk_peer_manager.clone();
                let tcp_stream = listener.accept().await.unwrap().0;
                tokio::spawn(async move {
                    let address = tcp_stream
                        .peer_addr()
                        .map(|a| a.to_string())
                        .unwrap_or_else(|_| "unknown".to_string());
                    let disconnected = lightning_net_tokio::setup_inbound(
                        peer_mgr.clone(),
                        tcp_stream.into_std().unwrap(),
                    );
                    info!("Inbound peer connection from {address}");
                    disconnected.await;
                    info!("Inbound peer disonnected from {address}");
                });
            }
        });
        Ok(())
    }

    pub async fn connect_peer(&self, public_key: PublicKey, peer_addr: SocketAddr) -> Result<()> {
        connect_peer(
            self.ldk_peer_manager.clone(),
            self.database.clone(),
            public_key,
            peer_addr,
        )
        .await
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
                    .filter(|id| !connected_node_ids.contains(id))
                {
                    match database.fetch_peer(&unconnected_node_id).await {
                        Ok(Some(peer)) => {
                            let _ = connect_peer(
                                ldk_peer_manager.clone(),
                                database.clone(),
                                peer.public_key,
                                peer.socket_addr,
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

    // Regularly broadcast our node_announcement. This is only required (or possible) if we have
    // some public channels, and is only useful if we have public listen address(es) to announce.
    // In a production environment, this should occur only after the announcement of new channels
    // to avoid churn in the global network graph.
    pub fn regularly_broadcast_node_announcement(&self) {
        let mut alias = [0; 32];
        alias[..self.settings.knd_node_name.len()]
            .copy_from_slice(self.settings.knd_node_name.as_bytes());
        let peer_manager = self.ldk_peer_manager.clone();
        if !self.settings.knd_listen_addresses.is_empty() {
            let addresses = self.settings.knd_listen_addresses.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(60));
                loop {
                    interval.tick().await;
                    peer_manager.broadcast_node_announcement(
                        [0; 3],
                        alias,
                        addresses.iter().map(|s| net_utils::to_address(s)).collect(),
                    );
                }
            });
        }
    }

    pub fn get_peer_node_ids(&self) -> Vec<PublicKey> {
        self.ldk_peer_manager.get_peer_node_ids()
    }

    pub async fn disconnect_by_node_id(
        &self,
        node_id: PublicKey,
        no_connection_possible: bool,
    ) -> Result<()> {
        self.ldk_peer_manager
            .disconnect_by_node_id(node_id, no_connection_possible);
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
    peer_addr: SocketAddr,
) -> Result<()> {
    let connection_closed =
        lightning_net_tokio::connect_outbound(ldk_peer_manager, public_key, peer_addr)
            .await
            .context("Could not connect to peer {public_key}@{peer_addr}")?;
    database
        .persist_peer(&database::peer::Peer {
            public_key,
            socket_addr: peer_addr,
        })
        .await?;
    info!("Connected to peer {public_key}@{peer_addr}");
    tokio::spawn(async move {
        connection_closed.await;
        info!("Disconnected from peer {public_key}@{peer_addr}");
    });
    Ok(())
}
