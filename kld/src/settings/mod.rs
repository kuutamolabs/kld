mod bitcoin_network;

use crate::api::NetAddress;
pub use bitcoin_network::Network;
use clap::{builder::OsStr, Parser};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Settings {
    #[arg(long, default_value = "localhost", env = "KLD_BITCOIN_RPC_HOST")]
    pub bitcoind_rpc_host: String,
    #[arg(long, default_value = "8333", env = "KLD_BITCOIN_RPC_PORT")]
    pub bitcoind_rpc_port: u16,
    #[arg(long, default_value = "regtest", env = "KLD_BITCOIN_NETWORK")]
    pub bitcoin_network: Network,
    #[arg(
        long,
        default_value = "/var/lib/bitcoind-testnet/.cookie",
        env = "KLD_BITCOIN_COOKIE_PATH"
    )]
    pub bitcoin_cookie_path: String,

    #[arg(long, default_value = "/var/lib/kld", env = "KLD_DATA_DIR")]
    pub data_dir: String,
    #[arg(long, default_value = "/var/lib/kld/certs", env = "KLD_CERTS_DIR")]
    pub certs_dir: String,
    #[arg(
        long,
        default_value = "/var/lib/kld/mnemonic",
        env = "KLD_MNEMONIC_PATH"
    )]
    pub mnemonic_path: String,
    #[arg(long, default_value = "one", env = "KLD_NODE_ID")]
    pub node_id: String,
    #[arg(long, default_value = "kld-wallet", env = "KLD_WALLET_NAME")]
    pub wallet_name: String,
    #[arg(long, default_value = "info", env = "KLD_LOG_LEVEL")]
    pub log_level: String,
    #[arg(long, default_value = "test", env = "KLD_ENV")]
    pub env: String,
    /// The port to listen to new peer connections on.
    #[arg(long, default_value = "9234", env = "KLD_PEER_PORT")]
    pub peer_port: u16,
    /// The node alias on the lightning network.
    #[arg(long, default_value = "testnode", env = "KLD_NODE_ALIAS")]
    pub node_alias: String,
    /// Public addresses to broadcast to the lightning network.
    #[arg(long, value_delimiter = ',', env = "KLD_PUBLIC_ADDRESSES")]
    pub public_addresses: Vec<NetAddress>,

    #[arg(long, default_value = "127.0.0.1:2233", env = "KLD_EXPORTER_ADDRESS")]
    pub exporter_address: String,
    #[arg(long, default_value = "0.0.0.0:2244", env = "KLD_REST_API_ADDRESS")]
    pub rest_api_address: String,

    #[arg(long, default_value = "127.0.0.1", env = "KLD_DATABASE_HOST")]
    pub database_host: String,
    #[arg(long, default_value = "10000", env = "KLD_DATABASE_PORT")]
    pub database_port: String,
    #[arg(long, default_value = "root", env = "KLD_DATABASE_USER")]
    pub database_user: String,
    #[arg(long, default_value = "defaultdb", env = "KLD_DATABASE_NAME")]
    pub database_name: String,
    #[arg(long, default_value = "", env = "KLD_DATABASE_CA_CERT_PATH")]
    pub database_ca_cert_path: String,
    #[arg(long, default_value = "", env = "KLD_DATABASE_CLIENT_CERT_PATH")]
    pub database_client_cert_path: String,
    #[arg(long, default_value = "", env = "KLD_DATABASE_CLIENT_KEY_PATH")]
    pub database_client_key_path: String,
}

impl Settings {
    pub fn load() -> Settings {
        Settings::parse()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings::parse_from::<Vec<OsStr>, OsStr>(vec![])
    }
}

#[cfg(test)]
mod test {
    use crate::settings::Settings;
    use std::env::set_var;

    #[test]
    pub fn test_parse_settings() {
        set_var("KLD_PUBLIC_ADDRESSES", "127.0.0.1:2312,[2001:db8::1]:1212");
        let settings = Settings::load();
        assert_eq!(settings.public_addresses.len(), 2);
    }
}
