mod bitcoin_network;

pub use crate::bitcoin_network::Network;
use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about, long_about = None)]
pub struct Settings {
    #[clap(long, default_value = "localhost", env = "KLD_BITCOIN_RPC_HOST")]
    pub bitcoind_rpc_host: String,
    #[clap(long, default_value = "8333", env = "KLD_BITCOIN_RPC_PORT")]
    pub bitcoind_rpc_port: u16,
    #[clap(long, default_value = "testnet", env = "KLD_BITCOIN_NETWORK")]
    pub bitcoin_network: Network,
    #[clap(
        long,
        default_value = "/var/lib/bitcoind-testnet/.cookie",
        env = "KLD_BITCOIN_COOKIE_PATH"
    )]
    pub bitcoin_cookie_path: String,

    #[clap(long, default_value = "/var/lib/kld", env = "KLD_DATA_DIR")]
    pub data_dir: String,
    #[clap(long, default_value = "/var/lib/kld/certs", env = "KLD_CERTS_DIR")]
    pub certs_dir: String,
    #[clap(
        long,
        default_value = "/var/lib/kld/mnemonic",
        env = "KLD_MNEMONIC_PATH"
    )]
    pub mnemonic_path: String,
    #[clap(long, default_value = "one", env = "KLD_NODE_ID")]
    pub node_id: String,
    #[clap(long, default_value = "info", env = "KLD_LOG_LEVEL")]
    pub log_level: String,
    #[clap(long, default_value = "test", env = "KLD_ENV")]
    pub env: String,
    /// The port to listen to new peer connections on.
    #[clap(long, default_value = "9234", env = "KLD_PEER_PORT")]
    pub peer_port: u16,
    /// The node alias on the lightning network.
    #[clap(long, default_value = "testnode", env = "KLD_NODE_NAME")]
    pub node_name: String,
    /// Listen addresses to broadcast to the lightning network.
    #[clap(long, default_value = "127.0.0.1:9234", env = "KLD_LISTEN_ADDRESSES")]
    pub listen_addresses: Vec<String>,

    #[clap(long, default_value = "127.0.0.1:2233", env = "KLD_EXPORTER_ADDRESS")]
    pub exporter_address: String,
    #[clap(long, default_value = "127.0.0.1:2244", env = "KLD_REST_API_ADDRESS")]
    pub rest_api_address: String,

    #[clap(long, default_value = "127.0.0.1", env = "KLD_DATABASE_HOST")]
    pub database_host: String,
    #[clap(long, default_value = "10000", env = "KLD_DATABASE_PORT")]
    pub database_port: String,
    #[clap(long, default_value = "root", env = "KLD_DATABASE_USER")]
    pub database_user: String,
    #[clap(long, default_value = "defaultdb", env = "KLD_DATABASE_NAME")]
    pub database_name: String,
    #[clap(long, default_value = "", env = "KLD_DATABASE_CA_CERT_PATH")]
    pub database_ca_cert_path: String,
    #[clap(long, default_value = "", env = "KLD_DATABASE_CLIENT_CERT_PATH")]
    pub database_client_cert_path: String,
    #[clap(long, default_value = "", env = "KLD_DATABASE_CLIENT_KEY_PATH")]
    pub database_client_key_path: String,
}

impl Settings {
    pub fn load() -> Settings {
        Settings::parse()
    }
}
