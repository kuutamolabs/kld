use std::{
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    str::FromStr,
};

use anyhow::{anyhow, Result};
use hex::ToHex;
use lightning::ln::msgs::NetAddress;

pub fn display_net_address(netaddr: &NetAddress) -> String {
    match netaddr {
        NetAddress::IPv4 { addr, port } => format!("{}:{port}", Ipv4Addr::from(*addr)),
        NetAddress::IPv6 { addr, port } => format!("{}:{port}", Ipv6Addr::from(*addr)),
        NetAddress::OnionV2(_) => "onionv2".to_string(),
        NetAddress::OnionV3 {
            ed25519_pubkey,
            checksum: _,
            version: _,
            port,
        } => format!("{}:{port}", ed25519_pubkey.encode_hex::<String>()),
        NetAddress::Hostname { hostname, port } => format!("{}:{port}", hostname.as_str()),
    }
}

pub fn parse_net_address(address: &str) -> Result<NetAddress> {
    if let Some((ip, port)) = address.rsplit_once(':') {
        if let Ok(ipv4) = Ipv4Addr::from_str(ip) {
            return Ok(NetAddress::IPv4 {
                addr: ipv4.octets(),
                port: port.parse()?,
            });
        } else if let Ok(ipv6) = Ipv6Addr::from_str(ip) {
            return Ok(NetAddress::IPv6 {
                addr: ipv6.octets(),
                port: port.parse()?,
            });
        }
    }
    Err(anyhow!("Invalid network address:port"))
}

pub fn to_socket_address(address: &NetAddress) -> Result<SocketAddr> {
    match address {
        NetAddress::IPv4 { addr, port } => Ok(SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::from(*addr),
            *port,
        ))),
        NetAddress::IPv6 { addr, port } => Ok(SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::from(*addr),
            *port,
            0,
            0,
        ))),
        _ => Err(anyhow!("unsupported address type")),
    }
}

#[test]
fn test_ipv4_net_address() -> Result<()> {
    let ipv4_address_str = "127.0.0.1:5050";
    let ipv4_address = NetAddress::IPv4 {
        addr: [127, 0, 0, 1],
        port: 5050,
    };
    let ipv4_socket = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 5050));

    assert_eq!(display_net_address(&ipv4_address), ipv4_address_str);
    assert_eq!(parse_net_address(ipv4_address_str)?, ipv4_address);
    assert_eq!(to_socket_address(&ipv4_address)?, ipv4_socket);
    Ok(())
}

#[test]
fn test_ipv6_net_address() -> Result<()> {
    let ipv6_address_str = "101:101:101:101:101:101:101:101:6060";
    let ipv6_address = NetAddress::IPv6 {
        addr: [1u8; 16],
        port: 6060,
    };
    assert_eq!(display_net_address(&ipv6_address), ipv6_address_str);
    assert_eq!(parse_net_address(ipv6_address_str)?, ipv6_address);
    Ok(())
}
