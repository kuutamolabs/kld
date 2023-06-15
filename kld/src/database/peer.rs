use anyhow::{anyhow, Result};
use api::lightning::{ln::msgs::NetAddress, util::ser::MaybeReadable};
use bitcoin::secp256k1::PublicKey;

#[derive(PartialEq, Eq, Debug)]
pub struct Peer {
    pub public_key: PublicKey,
    pub net_address: NetAddress,
}

impl Peer {
    pub fn deserialize(public_key: Vec<u8>, net_address: Vec<u8>) -> Result<Peer> {
        let public_key = PublicKey::from_slice(&public_key)?;
        let net_address = NetAddress::read(&mut net_address.as_slice())
            .map_err(|e| anyhow!("{}", e))?
            .ok_or(anyhow!("Error parsing address"))?;

        Ok(Peer {
            public_key,
            net_address,
        })
    }
}
