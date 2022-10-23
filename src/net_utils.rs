use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use bitcoin::secp256k1::PublicKey;
use lightning::ln::msgs::NetAddress;

use crate::controller::PeerManager;

pub fn to_address(address: &str) -> NetAddress {
    let (address, port) = address.split_once(':').unwrap();
    match IpAddr::from_str(address) {
        Ok(IpAddr::V4(a)) => NetAddress::IPv4 {
            addr: a.octets(),
            port: port.parse().unwrap(),
        },
        Ok(IpAddr::V6(a)) => NetAddress::IPv6 {
            addr: a.octets(),
            port: port.parse().unwrap(),
        },
        Err(_) => panic!("Unable to parse address"),
    }
}

pub(crate) async fn do_connect_peer(
    pubkey: PublicKey,
    peer_addr: SocketAddr,
    peer_manager: Arc<PeerManager>,
) -> Result<(), ()> {
    match lightning_net_tokio::connect_outbound(Arc::clone(&peer_manager), pubkey, peer_addr).await
    {
        Some(connection_closed_future) => {
            let mut connection_closed_future = Box::pin(connection_closed_future);
            loop {
                match futures::poll!(&mut connection_closed_future) {
                    std::task::Poll::Ready(_) => {
                        return Err(());
                    }
                    std::task::Poll::Pending => {}
                }
                // Avoid blocking the tokio context by sleeping a bit
                match peer_manager
                    .get_peer_node_ids()
                    .iter()
                    .find(|id| **id == pubkey)
                {
                    Some(_) => return Ok(()),
                    None => tokio::time::sleep(Duration::from_millis(10)).await,
                }
            }
        }
        None => Err(()),
    }
}
