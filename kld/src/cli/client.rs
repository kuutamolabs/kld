use std::{fs::File, io::Read};

use anyhow::{anyhow, Result};
use api::{
    routes, Channel, ChannelFee, FeeRate, FundChannel, FundChannelResponse, GetInfo, ListFunds,
    NetworkChannel, NetworkNode, NewAddress, NewAddressResponse, Peer, SetChannelFeeResponse,
    SignRequest, SignResponse, WalletBalance, WalletTransfer, WalletTransferResponse,
};
use bitcoin::secp256k1::PublicKey;
use reqwest::{
    blocking::{Client, ClientBuilder, RequestBuilder, Response},
    header::{HeaderValue, CONTENT_TYPE},
    Certificate, Method,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::to_string_pretty;

pub struct Api {
    host: String,
    client: Client,
    macaroon: Vec<u8>,
}

impl Api {
    pub fn new(host: &str, cert_path: &str, macaroon_path: &str) -> Result<Api> {
        let macaroon = read_file(macaroon_path)?;
        let cert = Certificate::from_pem(&read_file(cert_path)?)?;
        // Rustls does not support IP addresses (hostnames only) so we need to use native tls (openssl). Also turn off SNI as this requires host names as well.
        let client = ClientBuilder::new()
            .tls_sni(false)
            .add_root_certificate(cert)
            .use_native_tls()
            .timeout(None)
            .build()?;
        Ok(Api {
            host: host.to_string(),
            client,
            macaroon,
        })
    }

    pub fn sign(&self, message: String) -> Result<String> {
        let response = self
            .request_with_body(Method::POST, routes::SIGN, SignRequest { message })
            .send()?;
        deserialize::<SignResponse>(response)
    }

    pub fn get_info(&self) -> Result<String> {
        let response = self.request(Method::GET, routes::GET_INFO).send()?;
        deserialize::<GetInfo>(response)
    }

    pub fn get_balance(&self) -> Result<String> {
        let response = self.request(Method::GET, routes::GET_BALANCE).send()?;
        deserialize::<WalletBalance>(response)
    }

    pub fn new_address(&self) -> Result<String> {
        let response = self
            .request_with_body(Method::GET, routes::NEW_ADDR, NewAddress::default())
            .send()?;
        deserialize::<NewAddressResponse>(response)
    }

    pub fn withdraw(
        &self,
        address: String,
        satoshis: String,
        fee_rate: Option<FeeRate>,
    ) -> Result<String> {
        let wallet_transfer = WalletTransfer {
            address,
            satoshis,
            fee_rate,
            min_conf: None,
            utxos: vec![],
        };
        let response = self
            .request_with_body(Method::POST, routes::WITHDRAW, wallet_transfer)
            .send()?;
        deserialize::<WalletTransferResponse>(response)
    }

    pub fn list_funds(&self) -> Result<String> {
        let response = self.request(Method::GET, routes::LIST_FUNDS).send()?;
        deserialize::<ListFunds>(response)
    }

    pub fn list_channels(&self) -> Result<String> {
        let response = self.request(Method::GET, routes::LIST_CHANNELS).send()?;
        deserialize::<Vec<Channel>>(response)
    }

    pub fn list_peers(&self) -> Result<String> {
        let response = self.request(Method::GET, routes::LIST_PEERS).send()?;
        deserialize::<Vec<Peer>>(response)
    }

    pub fn connect_peer(&self, id: String) -> Result<String> {
        let response = self
            .request_with_body(Method::POST, routes::CONNECT_PEER, id)
            .send()?;
        deserialize::<PublicKey>(response)
    }

    pub fn disconnect_peer(&self, id: String) -> Result<String> {
        let response = self
            .request(Method::DELETE, &routes::DISCONNECT_PEER.replace(":id", &id))
            .send()?;
        deserialize::<()>(response)
    }

    pub fn open_channel(
        &self,
        id: String,
        satoshis: String,
        push_msat: Option<String>,
        announce: Option<bool>,
        fee_rate: Option<FeeRate>,
    ) -> Result<String> {
        let open_channel = FundChannel {
            id,
            satoshis,
            fee_rate,
            announce,
            min_conf: None,
            utxos: vec![],
            push_msat,
            close_to: None,
            request_amt: None,
            compact_lease: None,
        };
        let response = self
            .request_with_body(Method::POST, routes::OPEN_CHANNEL, open_channel)
            .send()?;
        deserialize::<FundChannelResponse>(response)
    }

    pub fn set_channel_fee(
        &self,
        id: String,
        base: Option<u32>,
        ppm: Option<u32>,
    ) -> Result<String> {
        let fee_request = ChannelFee { id, base, ppm };
        let response = self
            .request_with_body(Method::POST, routes::SET_CHANNEL_FEE, fee_request)
            .send()?;
        deserialize::<SetChannelFeeResponse>(response)
    }

    pub fn close_channel(&self, id: String) -> Result<String> {
        let response = self
            .request(Method::DELETE, &routes::CLOSE_CHANNEL.replace(":id", &id))
            .send()?;
        deserialize::<()>(response)
    }

    pub fn list_network_nodes(&self, id: Option<String>) -> Result<String> {
        let response = if let Some(id) = id {
            self.request(Method::GET, &routes::LIST_NETWORK_NODE.replace(":id", &id))
                .send()?
        } else {
            self.request(Method::GET, routes::LIST_NETWORK_NODES)
                .send()?
        };
        deserialize::<Vec<NetworkNode>>(response)
    }

    pub fn list_network_channels(&self, id: Option<String>) -> Result<String> {
        let response = if let Some(id) = id {
            self.request(
                Method::GET,
                &routes::LIST_NETWORK_CHANNEL.replace(":id", &id),
            )
            .send()?
        } else {
            self.request(Method::GET, routes::LIST_NETWORK_CHANNELS)
                .send()?
        };
        deserialize::<Vec<NetworkChannel>>(response)
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

fn deserialize<T: DeserializeOwned + Serialize>(response: Response) -> Result<String> {
    if response.status().is_success() {
        Ok(to_string_pretty(&response.json::<T>()?)?)
    } else {
        Ok(to_string_pretty(&response.json::<api::Error>()?)?)
    }
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
