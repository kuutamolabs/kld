use std::{fs::File, io::Read};

use anyhow::{anyhow, Result};
use api::{
    routes, Channel, ChannelFee, FundChannel, FundChannelResponse, GetInfo, NewAddress,
    NewAddressResponse, Node, Peer, SetChannelFeeResponse, WalletBalance, WalletTransfer,
    WalletTransferResponse,
};
use bitcoin::secp256k1::PublicKey;
use reqwest::{
    blocking::{Client, ClientBuilder, RequestBuilder},
    header::{HeaderValue, CONTENT_TYPE},
    Certificate, Method,
};
use serde::{de::DeserializeOwned, Serialize};

pub struct Api {
    host: String,
    client: Client,
    macaroon: Vec<u8>,
}

impl Api {
    pub fn new(host: &str, cert_path: &str, macaroon_path: &str) -> Result<Api> {
        let macaroon = read_file(macaroon_path)?;
        let cert = Certificate::from_pem(&read_file(cert_path)?)?;
        // Rustls does not support IP addresses (hostnames only) to we need to use native tls (openssl). Also turn off SNI as this requires host names as well.
        let client = ClientBuilder::new()
            .tls_sni(false)
            .add_root_certificate(cert)
            .use_native_tls()
            .build()?;
        Ok(Api {
            host: host.to_string(),
            client,
            macaroon,
        })
    }

    pub fn get_info(&self) -> Result<GetInfo> {
        send(self.request(Method::GET, routes::GET_INFO))
    }

    pub fn get_balance(&self) -> Result<WalletBalance> {
        send(self.request(Method::GET, routes::GET_BALANCE))
    }

    pub fn new_address(&self) -> Result<NewAddressResponse> {
        send(self.request_with_body(Method::GET, routes::NEW_ADDR, NewAddress::default()))
    }

    pub fn withdraw(&self, address: String, satoshis: String) -> Result<WalletTransferResponse> {
        let wallet_transfer = WalletTransfer {
            address,
            satoshis,
            fee_rate: None,
            min_conf: None,
            utxos: vec![],
        };
        send(self.request_with_body(Method::POST, routes::WITHDRAW, wallet_transfer))
    }

    pub fn list_channels(&self) -> Result<Vec<Channel>> {
        send(self.request(Method::GET, routes::LIST_CHANNELS))
    }

    pub fn list_peers(&self) -> Result<Vec<Peer>> {
        send(self.request(Method::GET, routes::LIST_PEERS))
    }

    pub fn connect_peer(&self, id: String) -> Result<PublicKey> {
        send(self.request_with_body(Method::POST, routes::CONNECT_PEER, id))
    }

    pub fn disconnect_peer(&self, id: String) -> Result<()> {
        send(self.request(Method::DELETE, &routes::DISCONNECT_PEER.replace(":id", &id)))
    }

    pub fn open_channel(
        &self,
        id: String,
        satoshis: String,
        push_msat: Option<String>,
    ) -> Result<FundChannelResponse> {
        let open_channel = FundChannel {
            id,
            satoshis,
            fee_rate: None,
            announce: None,
            min_conf: None,
            utxos: vec![],
            push_msat,
            close_to: None,
            request_amt: None,
            compact_lease: None,
        };
        send(self.request_with_body(Method::POST, routes::OPEN_CHANNEL, open_channel))
    }

    pub fn set_channel_fee(
        &self,
        id: String,
        base: Option<u32>,
        ppm: Option<u32>,
    ) -> Result<SetChannelFeeResponse> {
        let fee_request = ChannelFee { id, base, ppm };
        send(self.request_with_body(Method::POST, routes::SET_CHANNEL_FEE, fee_request))
    }

    pub fn close_channel(&self, id: String) -> Result<()> {
        send(self.request(Method::DELETE, &routes::CLOSE_CHANNEL.replace(":id", &id)))
    }

    pub fn list_nodes(&self, id: Option<String>) -> Result<Vec<Node>> {
        if let Some(id) = id {
            send(self.request(Method::GET, &routes::LIST_NODE.replace(":id", &id)))
        } else {
            send(self.request(Method::GET, routes::LIST_NODES))
        }
    }

    fn request_builder(&self, method: Method, route: &str) -> RequestBuilder {
        self.client
            .request(method, format!("https://{}{}", self.host, route))
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .header("macaroon", self.macaroon.clone())
    }

    fn request(&self, method: Method, route: &str) -> RequestBuilder {
        self.request_builder(method, route)
    }

    fn request_with_body<T: Serialize>(
        &self,
        method: Method,
        route: &str,
        body: T,
    ) -> RequestBuilder {
        let body = serde_json::to_string(&body).unwrap();
        self.request_builder(method, route).body(body)
    }
}

fn send<T: DeserializeOwned>(builder: RequestBuilder) -> Result<T> {
    let response = builder.send()?;
    if !response.status().is_success() {
        return Err(anyhow!("{}", response.status()));
    }
    Ok(response.json()?)
}

fn read_file(path: &str) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    match File::open(path) {
        Ok(mut file) => match file.read_to_end(&mut buf) {
            Ok(_) => Ok(buf),
            Err(e) => Err(anyhow!("{}: {}", e, path)),
        },
        Err(e) => Err(anyhow!("{}: {}", e, path)),
    }
}
