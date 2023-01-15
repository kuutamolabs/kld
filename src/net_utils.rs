use std::{net::IpAddr, str::FromStr};

use lightning::ln::msgs::NetAddress;

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
