use std::net::{SocketAddr, ToSocketAddrs};

use anyhow::anyhow;
use bitcoin::secp256k1::PublicKey;

#[derive(PartialEq, Eq, Debug)]
pub struct Peer {
    pub public_key: PublicKey,
    pub socket_addr: SocketAddr,
}

impl TryFrom<(String, Vec<u8>)> for Peer {
    type Error = anyhow::Error;

    fn try_from((s1, s2): (String, Vec<u8>)) -> Result<Self, Self::Error> {
        let public_key = to_compressed_pubkey(&s1)
            .ok_or_else(|| anyhow!("Failed to parse public_key {}", s1))?;
        let socket_addr = String::from_utf8(s2)?
            .to_socket_addrs()?
            .last()
            .ok_or_else(|| anyhow!("No address found"))?;
        Ok(Peer {
            public_key,
            socket_addr,
        })
    }
}

fn to_compressed_pubkey(hex: &str) -> Option<PublicKey> {
    if hex.len() != 33 * 2 {
        return None;
    }
    let data = match to_vec(&hex[0..33 * 2]) {
        Some(bytes) => bytes,
        None => return None,
    };
    match PublicKey::from_slice(&data) {
        Ok(pk) => Some(pk),
        Err(_) => None,
    }
}

fn to_vec(hex: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(hex.len() / 2);

    let mut b = 0;
    for (idx, c) in hex.as_bytes().iter().enumerate() {
        b <<= 4;
        match *c {
            b'A'..=b'F' => b |= c - b'A' + 10,
            b'a'..=b'f' => b |= c - b'a' + 10,
            b'0'..=b'9' => b |= c - b'0',
            _ => return None,
        }
        if (idx & 1) == 1 {
            out.push(b);
            b = 0;
        }
    }

    Some(out)
}
