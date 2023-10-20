use anyhow::{anyhow, Result};
use bitcoin::secp256k1::PublicKey;
use lightning::{ln::msgs::SocketAddress, util::ser::MaybeReadable};

#[derive(PartialEq, Eq, Debug)]
pub struct Peer {
    pub public_key: PublicKey,
    pub address: SocketAddress,
}

impl Peer {
    pub fn deserialize(public_key: Vec<u8>, net_address: Vec<u8>) -> Result<Peer> {
        let public_key = PublicKey::from_slice(&public_key)?;
        let address = SocketAddress::read(&mut net_address.as_slice())
            .map_err(|e| anyhow!("{}", e))?
            .ok_or(anyhow!("Error parsing address"))?;

        Ok(Peer {
            public_key,
            address,
        })
    }
}
