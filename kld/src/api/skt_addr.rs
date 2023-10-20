use core::ops::Deref;
use std::{
    fmt::Display,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    str::FromStr,
};

pub use lightning;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A wrapper for lightning::ln::msgs::SocketAddress
#[derive(Debug, PartialEq, Clone)]
pub struct SocketAddress(pub lightning::ln::msgs::SocketAddress);

impl SocketAddress {
    pub fn is_ipv4(&self) -> bool {
        matches!(self.0, lightning::ln::msgs::SocketAddress::TcpIpV4 { .. })
    }

    pub fn is_ipv6(&self) -> bool {
        matches!(self.0, lightning::ln::msgs::SocketAddress::TcpIpV6 { .. })
    }

    pub fn inner(self) -> lightning::ln::msgs::SocketAddress {
        self.0
    }
}

impl Deref for SocketAddress {
    type Target = lightning::ln::msgs::SocketAddress;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Serialize, Deserialize)]
pub enum SocketAddressHelper {
    IPv4(([u8; 4], u16)),
    IPv6(([u8; 16], u16)),
    OnionV2([u8; 12]),
    // key , checksum, version, port
    OnionV3(([u8; 32], u16, u8, u16)),
    // port, hostname
    Hostname((u16, String)),
}

// Drop this when PR merge
// https://github.com/lightningdevkit/rust-lightning/pull/2670
impl Display for SocketAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            lightning::ln::msgs::SocketAddress::TcpIpV4 { addr, port } => {
                write!(f, "{}:{port}", Ipv4Addr::from(*addr))?
            }
            lightning::ln::msgs::SocketAddress::TcpIpV6 { addr, port } => {
                write!(f, "[{}]:{port}", Ipv6Addr::from(*addr))?
            }
            lightning::ln::msgs::SocketAddress::OnionV2(bytes) => write!(f, "{bytes:?}.onion")?,
            lightning::ln::msgs::SocketAddress::OnionV3 {
                port,
                ed25519_pubkey,
                ..
            } => write!(f, "{ed25519_pubkey:?}.onion:{port}")?,
            lightning::ln::msgs::SocketAddress::Hostname { hostname, port } => {
                write!(f, "{hostname:?}:{port}")?
            }
        }
        Ok(())
    }
}

impl Serialize for SocketAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let net_address_type = match &self.0 {
            lightning::ln::msgs::SocketAddress::TcpIpV4 { addr, port } => {
                SocketAddressHelper::IPv4((*addr, *port))
            }
            lightning::ln::msgs::SocketAddress::TcpIpV6 { addr, port } => {
                SocketAddressHelper::IPv6((*addr, *port))
            }
            lightning::ln::msgs::SocketAddress::OnionV2(bytes) => {
                SocketAddressHelper::OnionV2(*bytes)
            }
            lightning::ln::msgs::SocketAddress::OnionV3 {
                ed25519_pubkey,
                checksum,
                version,
                port,
            } => SocketAddressHelper::OnionV3((*ed25519_pubkey, *checksum, *version, *port)),
            lightning::ln::msgs::SocketAddress::Hostname { hostname, port } => {
                SocketAddressHelper::Hostname((*port, hostname.to_string()))
            }
        };
        net_address_type.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SocketAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer).map(|helper| {
            let inner = match helper {
                SocketAddressHelper::IPv4((addr, port)) => {
                    lightning::ln::msgs::SocketAddress::TcpIpV4 { addr, port }
                }
                SocketAddressHelper::IPv6((addr, port)) => {
                    lightning::ln::msgs::SocketAddress::TcpIpV6 { addr, port }
                }
                SocketAddressHelper::OnionV2(bytes) => {
                    lightning::ln::msgs::SocketAddress::OnionV2(bytes)
                }
                SocketAddressHelper::OnionV3((ed25519_pubkey, checksum, version, port)) => {
                    lightning::ln::msgs::SocketAddress::OnionV3 {
                        ed25519_pubkey,
                        checksum,
                        version,
                        port,
                    }
                }
                SocketAddressHelper::Hostname((port, hostname)) => {
                    let hostname = lightning::util::ser::Hostname::try_from(hostname.clone())
                        .unwrap_or_else(|_| {
                            eprintln!("invalid hostname detected: {:?}", hostname);
                            lightning::util::ser::Hostname::try_from("".to_string())
                                .expect("Replcing invalid hostname with empty one")
                        });
                    lightning::ln::msgs::SocketAddress::Hostname { hostname, port }
                }
            };
            SocketAddress(inner)
        })
    }
}

impl From<lightning::ln::msgs::SocketAddress> for SocketAddress {
    fn from(inner: lightning::ln::msgs::SocketAddress) -> Self {
        Self(inner)
    }
}

impl From<SocketAddr> for SocketAddress {
    fn from(addr: SocketAddr) -> Self {
        SocketAddress(addr.into())
    }
}

impl From<SocketAddrV4> for SocketAddress {
    fn from(v4: SocketAddrV4) -> Self {
        SocketAddress(v4.into())
    }
}

impl From<SocketAddrV6> for SocketAddress {
    fn from(v6: SocketAddrV6) -> Self {
        SocketAddress(v6.into())
    }
}

impl FromStr for SocketAddress {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if let Ok(addr) = lightning::ln::msgs::SocketAddress::from_str(s) {
            Ok(Self(addr))
        } else {
            anyhow::bail!("{} is not a valid socket address", s)
        }
    }
}

impl TryFrom<SocketAddress> for SocketAddr {
    type Error = anyhow::Error;

    fn try_from(address: SocketAddress) -> std::result::Result<Self, Self::Error> {
        match address.0 {
            lightning::ln::msgs::SocketAddress::TcpIpV4 { addr, port } => Ok(SocketAddr::V4(
                SocketAddrV4::new(Ipv4Addr::from(addr), port),
            )),
            lightning::ln::msgs::SocketAddress::TcpIpV6 { addr, port } => Ok(SocketAddr::V6(
                SocketAddrV6::new(Ipv6Addr::from(addr), port, 0, 0),
            )),
            _ => anyhow::bail!("unsupported address type"),
        }
    }
}

#[test]
fn test_netaddress() {
    let v4_addr = SocketAddress(lightning::ln::msgs::SocketAddress::TcpIpV4 {
        addr: Ipv4Addr::new(127, 0, 0, 1).octets(),
        port: 80,
    });
    let mut bytes = bincode::serialize(&v4_addr).unwrap();
    let v4_decoded: SocketAddress = bincode::deserialize(&bytes).unwrap();
    assert_eq!(v4_addr, v4_decoded);

    let v6_addr = SocketAddress(lightning::ln::msgs::SocketAddress::TcpIpV6 {
        addr: Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1).octets(),
        port: 80,
    });
    bytes = bincode::serialize(&v6_addr).unwrap();
    let v6_decoded: SocketAddress = bincode::deserialize(&bytes).unwrap();
    assert_eq!(v6_addr, v6_decoded);
}
