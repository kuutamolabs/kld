use core::ops::Deref;
use std::{
    fmt::Display,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    str::FromStr,
};

use hex::ToHex;
pub use lightning;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A wrapper for lightning::ln::msgs::NetAddress
#[derive(Debug, PartialEq, Clone)]
pub struct NetAddress(pub lightning::ln::msgs::NetAddress);

impl NetAddress {
    pub fn is_ipv4(&self) -> bool {
        matches!(self.0, lightning::ln::msgs::NetAddress::IPv4 { .. })
    }

    pub fn is_ipv6(&self) -> bool {
        matches!(self.0, lightning::ln::msgs::NetAddress::IPv6 { .. })
    }

    pub fn inner(self) -> lightning::ln::msgs::NetAddress {
        self.0
    }
}

impl Deref for NetAddress {
    type Target = lightning::ln::msgs::NetAddress;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Serialize, Deserialize)]
pub enum NetAddressHelper {
    IPv4(([u8; 4], u16)),
    IPv6(([u8; 16], u16)),
    OnionV2([u8; 12]),
    // key , checksum, version, port
    OnionV3(([u8; 32], u16, u8, u16)),
    // port, hostname
    Hostname((u16, String)),
}

impl Display for NetAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            lightning::ln::msgs::NetAddress::IPv4 { addr, port } => {
                write!(f, "{}:{port}", Ipv4Addr::from(*addr))?
            }
            lightning::ln::msgs::NetAddress::IPv6 { addr, port } => {
                write!(f, "[{}]:{port}", Ipv6Addr::from(*addr))?
            }
            lightning::ln::msgs::NetAddress::OnionV2(bytes) => {
                write!(f, "onionv2({})", bytes.encode_hex::<String>())?
            }
            lightning::ln::msgs::NetAddress::OnionV3 {
                ed25519_pubkey,
                checksum: _,
                version: _,
                port,
            } => write!(f, "{}:{port}", ed25519_pubkey.encode_hex::<String>())?,
            lightning::ln::msgs::NetAddress::Hostname { hostname, port } => {
                write!(f, "{}:{port}", hostname.as_str())?
            }
        };
        Ok(())
    }
}

impl Serialize for NetAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let net_address_type = match &self.0 {
            lightning::ln::msgs::NetAddress::IPv4 { addr, port } => {
                NetAddressHelper::IPv4((*addr, *port))
            }
            lightning::ln::msgs::NetAddress::IPv6 { addr, port } => {
                NetAddressHelper::IPv6((*addr, *port))
            }
            lightning::ln::msgs::NetAddress::OnionV2(bytes) => NetAddressHelper::OnionV2(*bytes),
            lightning::ln::msgs::NetAddress::OnionV3 {
                ed25519_pubkey,
                checksum,
                version,
                port,
            } => NetAddressHelper::OnionV3((*ed25519_pubkey, *checksum, *version, *port)),
            lightning::ln::msgs::NetAddress::Hostname { hostname, port } => {
                NetAddressHelper::Hostname((*port, hostname.to_string()))
            }
        };
        net_address_type.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NetAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer).map(|helper| {
            let inner = match helper {
                NetAddressHelper::IPv4((addr, port)) => {
                    lightning::ln::msgs::NetAddress::IPv4 { addr, port }
                }
                NetAddressHelper::IPv6((addr, port)) => {
                    lightning::ln::msgs::NetAddress::IPv6 { addr, port }
                }
                NetAddressHelper::OnionV2(bytes) => lightning::ln::msgs::NetAddress::OnionV2(bytes),
                NetAddressHelper::OnionV3((ed25519_pubkey, checksum, version, port)) => {
                    lightning::ln::msgs::NetAddress::OnionV3 {
                        ed25519_pubkey,
                        checksum,
                        version,
                        port,
                    }
                }
                NetAddressHelper::Hostname((port, hostname)) => {
                    let hostname = lightning::util::ser::Hostname::try_from(hostname.clone())
                        .unwrap_or_else(|_| {
                            eprintln!("invalid hostname detected: {:?}", hostname);
                            lightning::util::ser::Hostname::try_from("".to_string())
                                .expect("Replcing invalid hostname with empty one")
                        });
                    lightning::ln::msgs::NetAddress::Hostname { hostname, port }
                }
            };
            NetAddress(inner)
        })
    }
}

impl From<lightning::ln::msgs::NetAddress> for NetAddress {
    fn from(inner: lightning::ln::msgs::NetAddress) -> Self {
        Self(inner)
    }
}

impl From<SocketAddr> for NetAddress {
    fn from(addr: SocketAddr) -> Self {
        match addr {
            SocketAddr::V4(v4) => NetAddress(lightning::ln::msgs::NetAddress::IPv4 {
                addr: v4.ip().octets(),
                port: v4.port(),
            }),
            SocketAddr::V6(v6) => NetAddress(lightning::ln::msgs::NetAddress::IPv6 {
                addr: v6.ip().octets(),
                port: v6.port(),
            }),
        }
    }
}

impl From<SocketAddrV4> for NetAddress {
    fn from(v4: SocketAddrV4) -> Self {
        NetAddress(lightning::ln::msgs::NetAddress::IPv4 {
            addr: v4.ip().octets(),
            port: v4.port(),
        })
    }
}

impl From<SocketAddrV6> for NetAddress {
    fn from(v6: SocketAddrV6) -> Self {
        NetAddress(lightning::ln::msgs::NetAddress::IPv6 {
            addr: v6.ip().octets(),
            port: v6.port(),
        })
    }
}

impl FromStr for NetAddress {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if let Ok(sktv4) = SocketAddrV4::from_str(s) {
            return Ok(sktv4.into());
        }
        if let Ok(sktv6) = SocketAddrV6::from_str(s) {
            return Ok(sktv6.into());
        }
        anyhow::bail!("Invalid or unsupported network address: {s:?}")
    }
}

impl TryFrom<NetAddress> for SocketAddr {
    type Error = anyhow::Error;

    fn try_from(address: NetAddress) -> std::result::Result<Self, Self::Error> {
        match address.0 {
            lightning::ln::msgs::NetAddress::IPv4 { addr, port } => Ok(SocketAddr::V4(
                SocketAddrV4::new(Ipv4Addr::from(addr), port),
            )),
            lightning::ln::msgs::NetAddress::IPv6 { addr, port } => Ok(SocketAddr::V6(
                SocketAddrV6::new(Ipv6Addr::from(addr), port, 0, 0),
            )),
            _ => anyhow::bail!("unsupported address type"),
        }
    }
}

#[test]
fn test_netaddress() {
    let v4_addr = NetAddress(lightning::ln::msgs::NetAddress::IPv4 {
        addr: Ipv4Addr::new(127, 0, 0, 1).octets(),
        port: 80,
    });
    let mut bytes = bincode::serialize(&v4_addr).unwrap();
    let v4_decoded: NetAddress = bincode::deserialize(&bytes).unwrap();
    assert_eq!(v4_addr, v4_decoded);

    let v6_addr = NetAddress(lightning::ln::msgs::NetAddress::IPv6 {
        addr: Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1).octets(),
        port: 80,
    });
    bytes = bincode::serialize(&v6_addr).unwrap();
    let v6_decoded: NetAddress = bincode::deserialize(&bytes).unwrap();
    assert_eq!(v6_addr, v6_decoded);
}
