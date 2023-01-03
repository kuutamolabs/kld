use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct GetInfo {
    #[serde(rename = "id")]
    pub identity_pubkey: String,
    pub alias: String,
    pub num_pending_channels: usize,
    pub num_active_channels: usize,
    pub num_inactive_channels: usize,
    pub num_peers: usize,
    #[serde(rename = "blockheight")]
    pub block_height: usize,
    pub synced_to_chain: bool,
    pub testnet: bool,
    pub chains: Vec<Chain>,
    pub version: String,
}

#[derive(Serialize, Deserialize)]
pub struct Chain {
    pub chain: String,
    pub network: String,
}

#[derive(Serialize, Deserialize)]
pub struct Balance {
    #[serde(rename = "totalBalance")]
    pub total_balance: u64,
    #[serde(rename = "confBlance")]
    pub conf_balance: u64,
    #[serde(rename = "unconfBalance")]
    pub unconf_balance: u64,
}
