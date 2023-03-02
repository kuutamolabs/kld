use std::{
    fmt::Display,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    str::FromStr,
};

use anyhow::anyhow;
use hex::ToHex;
use lightning::ln::msgs::NetAddress;

#[derive(Debug, PartialEq, Clone)]
pub struct PeerAddress(pub NetAddress);

impl Display for PeerAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            NetAddress::IPv4 { addr, port } => write!(f, "{}:{port}", Ipv4Addr::from(*addr))?,
            NetAddress::IPv6 { addr, port } => write!(f, "{}:{port}", Ipv6Addr::from(*addr))?,
            NetAddress::OnionV2(_) => write!(f, "onionv2")?,
            NetAddress::OnionV3 {
                ed25519_pubkey,
                checksum: _,
                version: _,
                port,
            } => write!(f, "{}:{port}", ed25519_pubkey.encode_hex::<String>())?,
            NetAddress::Hostname { hostname, port } => write!(f, "{}:{port}", hostname.as_str())?,
        };
        Ok(())
    }
}

impl FromStr for PeerAddress {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if let Some((ip, port)) = s.rsplit_once(':') {
            if let Ok(ipv4) = Ipv4Addr::from_str(ip) {
                return Ok(PeerAddress(NetAddress::IPv4 {
                    addr: ipv4.octets(),
                    port: port.parse()?,
                }));
            } else if let Ok(ipv6) = Ipv6Addr::from_str(ip) {
                return Ok(PeerAddress(NetAddress::IPv6 {
                    addr: ipv6.octets(),
                    port: port.parse()?,
                }));
            }
        }
        Err(anyhow!("Invalid network address:port"))
    }
}

impl TryFrom<PeerAddress> for SocketAddr {
    type Error = anyhow::Error;

    fn try_from(peer_address: PeerAddress) -> std::result::Result<Self, Self::Error> {
        match peer_address.0 {
            NetAddress::IPv4 { addr, port } => Ok(SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::from(addr),
                port,
            ))),
            NetAddress::IPv6 { addr, port } => Ok(SocketAddr::V6(SocketAddrV6::new(
                Ipv6Addr::from(addr),
                port,
                0,
                0,
            ))),
            _ => Err(anyhow!("unsupported address type")),
        }
    }
}

#[test]
fn test_ipv4_net_address() -> anyhow::Result<()> {
    let ipv4_address_str = "127.0.0.1:5050";
    let ipv4_address = PeerAddress(NetAddress::IPv4 {
        addr: [127, 0, 0, 1],
        port: 5050,
    });
    let ipv4_socket = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 5050));
    assert_eq!(&ipv4_address.to_string(), ipv4_address_str);
    assert_eq!(ipv4_address_str.parse::<PeerAddress>()?, ipv4_address);
    assert_eq!(SocketAddr::try_from(ipv4_address)?, ipv4_socket);
    Ok(())
}

#[test]
fn test_ipv6_net_address() -> anyhow::Result<()> {
    let ipv6_address_str = "101:101:101:101:101:101:101:101:6060";
    let ipv6_address = PeerAddress(NetAddress::IPv6 {
        addr: [1u8; 16],
        port: 6060,
    });
    assert_eq!(&ipv6_address.to_string(), ipv6_address_str);
    assert_eq!(ipv6_address_str.parse::<PeerAddress>()?, ipv6_address);
    Ok(())
}
