use std::{
    fs::{self},
    net::SocketAddr,
    path::PathBuf,
    str::FromStr,
};

use anyhow::Result;
use api::{
    routes, ChannelFee, FeeRate, FeeRatesResponse, FundChannel, FundChannelResponse,
    GenerateInvoice, GenerateInvoiceResponse, GetInfo, Invoice, KeysendRequest, ListFunds,
    NetworkChannel, NetworkNode, PayInvoice, PaymentResponse, Peer, SetChannelFeeResponse,
    SignRequest, SignResponse, WalletBalance, WalletTransfer, WalletTransferResponse,
};
use kld::api::codegen::{
    get_v1_channel_history_response::GetV1ChannelHistoryResponseItem,
    get_v1_channel_list_forwards_response::GetV1ChannelListForwardsResponseItem,
    get_v1_channel_list_peer_channels_response::GetV1ChannelListPeerChannelsResponse,
    get_v1_channel_localremotebal_response::GetV1ChannelLocalremotebalResponse,
    get_v1_estimate_channel_liquidity_body::GetV1EstimateChannelLiquidityBody,
    get_v1_estimate_channel_liquidity_response::GetV1EstimateChannelLiquidityResponse,
    get_v1_get_fees_response::GetV1GetFeesResponse, get_v1_newaddr_response::GetV1NewaddrResponse,
    get_v1_pay_list_payments_response::GetV1PayListPaymentsResponse,
    get_v1_utility_decode_invoice_string_response::GetV1UtilityDecodeInvoiceStringResponse,
    post_v1_peer_connect_body::PostV1PeerConnectBody,
    post_v1_peer_connect_response::PostV1PeerConnectResponse,
};
use reqwest::{
    blocking::{Client, ClientBuilder, RequestBuilder, Response},
    header::{HeaderValue, CONTENT_TYPE},
    Certificate, Method,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::to_string_pretty;

pub struct Api {
    host: SocketAddr,
    client: Client,
    macaroon: Vec<u8>,
}

impl Api {
    pub fn new(host: SocketAddr, cert_path: PathBuf, macaroon_path: PathBuf) -> Result<Api> {
        let macaroon = fs::read(macaroon_path)?;
        let cert = Certificate::from_pem(&fs::read(cert_path)?)?;
        // Rustls does not support IP addresses (hostnames only) so we need to use native tls (openssl)
        let client = ClientBuilder::new()
            .add_root_certificate(cert)
            .use_native_tls()
            .timeout(None)
            .build()?;
        Ok(Api {
            host,
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
        let response = self.request(Method::GET, routes::NEW_ADDR).send()?;
        deserialize::<GetV1NewaddrResponse>(response)
    }

    pub fn withdraw(
        &self,
        address: String,
        satoshis: String,
        fee_rate: Option<String>,
    ) -> Result<String> {
        let wallet_transfer = WalletTransfer {
            address,
            satoshis,
            fee_rate: fee_rate.map(|f| FeeRate::from_str(&f)).transpose()?,
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

    pub fn list_peer_channels(&self) -> Result<String> {
        let response = self
            .request(Method::GET, routes::LIST_PEER_CHANNELS)
            .send()?;
        deserialize::<Vec<GetV1ChannelListPeerChannelsResponse>>(response)
    }

    pub fn list_peers(&self) -> Result<String> {
        let response = self.request(Method::GET, routes::LIST_PEERS).send()?;
        deserialize::<Vec<Peer>>(response)
    }

    pub fn connect_peer(&self, id: String) -> Result<String> {
        let connect = PostV1PeerConnectBody { id };
        let response = self
            .request_with_body(Method::POST, routes::CONNECT_PEER, connect)
            .send()?;
        deserialize::<PostV1PeerConnectResponse>(response)
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
        fee_rate: Option<String>,
    ) -> Result<String> {
        let open_channel = FundChannel {
            id,
            satoshis,
            fee_rate: fee_rate.map(|f| FeeRate::from_str(&f)).transpose()?,
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

    pub fn fee_rates(&self, style: Option<String>) -> Result<String> {
        let response = self
            .request(
                Method::GET,
                &routes::FEE_RATES.replace(":style", &style.unwrap_or("perkb".to_string())),
            )
            .send()?;
        deserialize::<FeeRatesResponse>(response)
    }

    pub fn keysend(&self, public_key: String, amount: u64) -> Result<String> {
        let body = KeysendRequest {
            pubkey: public_key,
            amount,
            label: None,
            maxfeepercent: None,
            retry_for: None,
            maxdelay: None,
            exemptfee: None,
        };
        let response = self
            .request_with_body(Method::POST, routes::KEYSEND, body)
            .send()?;
        deserialize::<PaymentResponse>(response)
    }

    pub fn generate_invoice(
        &self,
        amount: u64,
        label: String,
        description: String,
        expiry: Option<u32>,
    ) -> Result<String> {
        let body = GenerateInvoice {
            amount,
            label,
            description,
            expiry,
            ..Default::default()
        };
        let response = self
            .request_with_body(Method::POST, routes::GENERATE_INVOICE, body)
            .send()?;
        deserialize::<GenerateInvoiceResponse>(response)
    }

    pub fn list_invoices(&self, label: Option<String>) -> Result<String> {
        let route = if let Some(label) = label {
            format!("{}?{label}", routes::LIST_INVOICES)
        } else {
            routes::LIST_INVOICES.to_string()
        };
        let response = self.request(Method::GET, &route).send()?;
        deserialize::<Vec<Invoice>>(response)
    }

    pub fn pay_invoice(&self, bolt11: String, label: Option<String>) -> Result<String> {
        let body = PayInvoice {
            invoice: bolt11,
            label,
        };
        let response = self
            .request_with_body(Method::POST, routes::PAY_INVOICE, body)
            .send()?;
        deserialize::<PaymentResponse>(response)
    }

    pub fn list_payments(
        &self,
        bolt11: Option<String>,
        direction: Option<String>,
    ) -> Result<String> {
        let mut params = vec![];
        if let Some(bolt11) = bolt11 {
            params.push(("bolt11", bolt11));
        }
        if let Some(direction) = direction {
            params.push(("direction", direction));
        }
        let response = self
            .request(Method::GET, routes::LIST_PAYMENTS)
            .query(&params)
            .send()?;
        deserialize::<GetV1PayListPaymentsResponse>(response)
    }

    pub fn estimate_channel_liquidity(&self, scid: u64, target: String) -> Result<String> {
        let body = GetV1EstimateChannelLiquidityBody {
            scid: scid as i64,
            target,
        };
        let response = self
            .request_with_body(Method::GET, routes::ESTIMATE_CHANNEL_LIQUIDITY, body)
            .send()?;
        deserialize::<GetV1EstimateChannelLiquidityResponse>(response)
    }

    pub fn local_remote_balance(&self) -> Result<String> {
        let response = self
            .request(Method::GET, routes::LOCAL_REMOTE_BALANCE)
            .send()?;
        deserialize::<GetV1ChannelLocalremotebalResponse>(response)
    }

    pub fn get_fees(&self) -> Result<String> {
        let response = self.request(Method::GET, routes::GET_FEES).send()?;
        deserialize::<GetV1GetFeesResponse>(response)
    }

    pub fn list_forwards(&self, status: Option<String>) -> Result<String> {
        let mut params = vec![];
        if let Some(status) = status {
            params.push(("status", status));
        }
        let response = self
            .request(Method::GET, routes::LIST_FORWARDS)
            .query(&params)
            .send()?;
        deserialize::<Vec<GetV1ChannelListForwardsResponseItem>>(response)
    }

    pub fn channel_history(&self) -> Result<String> {
        let response = self
            .request(Method::GET, routes::LIST_CHANNEL_HISTORY)
            .send()?;
        deserialize::<Vec<GetV1ChannelHistoryResponseItem>>(response)
    }

    pub fn decode(&self, invoice: String) -> Result<String> {
        let response = self
            .request(
                Method::GET,
                &routes::DECODE_INVOICE.replace(":invoice", &invoice),
            )
            .send()?;
        deserialize::<GetV1UtilityDecodeInvoiceStringResponse>(response)
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
