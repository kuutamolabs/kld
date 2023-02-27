use api::{Address, Node};
use axum::{response::IntoResponse, Extension, Json};
use bitcoin::secp256k1::PublicKey;
use hex::ToHex;
use hyper::StatusCode;
use lightning::ln::msgs::NetAddress;
use log::info;
use std::{
    net::{Ipv4Addr, Ipv6Addr},
    sync::Arc,
};

use crate::handle_unauthorized;

use super::{KndMacaroon, LightningInterface, MacaroonAuth};

pub(crate) async fn list_node(
    macaroon: KndMacaroon,
    Extension(macaroon_auth): Extension<Arc<MacaroonAuth>>,
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Json(public_key): Json<PublicKey>,
) -> Result<impl IntoResponse, StatusCode> {
    handle_unauthorized!(macaroon_auth.verify_readonly_macaroon(&macaroon.0));

    if let Some(node_info) = lightning_interface.list_node(&public_key) {
        if let Some(announcement) = &node_info.announcement_info {
            let node = Node {
                node_id: public_key.to_string(),
                alias: announcement.alias.to_string(),
                color: announcement.rgb.encode_hex(),
                last_timestamp: announcement.last_update,
                features: announcement.features.to_string(),
                addresses: announcement.addresses.iter().map(to_api_address).collect(),
            };
            return Ok(Json(vec![node]));
        }
    }
    Err(StatusCode::NOT_FOUND)
}

fn to_api_address(net_address: &NetAddress) -> Address {
    match net_address {
        NetAddress::IPv4 { addr, port } => Address {
            address_type: "ipv4".to_string(),
            address: Ipv4Addr::from(*addr).to_string(),
            port: *port,
        },
        NetAddress::IPv6 { addr, port } => Address {
            address_type: "ipv6".to_string(),
            address: Ipv6Addr::from(*addr).to_string(),
            port: *port,
        },
        NetAddress::OnionV2(pubkey) => Address {
            address_type: "onionv2".to_string(),
            address: pubkey.encode_hex(),
            port: 0,
        },
        NetAddress::OnionV3 {
            ed25519_pubkey,
            checksum: _,
            version: _,
            port,
        } => Address {
            address_type: "onionv3".to_string(),
            address: ed25519_pubkey.encode_hex(),
            port: *port,
        },
        NetAddress::Hostname { hostname, port } => Address {
            address_type: "hostname".to_string(),
            address: hostname.to_string(),
            port: *port,
        },
    }
}
