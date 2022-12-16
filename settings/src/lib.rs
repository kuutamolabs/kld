use bitcoin::Network;
use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about, long_about = None)]
pub struct Settings {
    #[clap(long, default_value = "localhost", env = "KND_BITCOIN_RPC_HOST")]
    pub bitcoind_rpc_host: String,
    #[clap(long, default_value = "8333", env = "KND_BITCOIN_RPC_PORT")]
    pub bitcoind_rpc_port: u16,
    #[clap(long, default_value = "testnet", env = "KND_BITCOIN_NETWORK")]
    pub bitcoin_network: Network,
    #[clap(long, default_value = "testnet", env = "KND_BITCOIN_COOKIE_PATH")]
    pub bitcoin_cookie_path: String,

    #[clap(long, default_value = "/var/lib/knd", env = "KND_DATA_DIR")]
    pub data_dir: String,
    #[clap(long, default_value = "one", env = "KND_NODE_ID")]
    pub node_id: String,
    #[clap(long, default_value = "info", env = "KND_LOG_LEVEL")]
    pub log_level: String,
    #[clap(long, default_value = "test", env = "KND_ENV")]
    pub env: String,
    #[clap(long, default_value = "9234", env = "KND_PEER_PORT")]
    pub knd_peer_port: u16,
    #[clap(long, default_value = "testnode", env = "KND_NODE_NAME")]
    pub knd_node_name: String,
    #[clap(long, default_value = "127.0.0.1:9234", env = "KND_LISTEN_ADDRESSES")]
    pub knd_listen_addresses: Vec<String>,

    #[clap(long, default_value = "127.0.0.1:2233", env = "KND_EXPORTER_ADDRESS")]
    pub exporter_address: String,
    #[clap(long, default_value = "127.0.0.1:2244", env = "KND_REST_API_ADDRESS")]
    pub rest_api_address: String,

    #[clap(long, default_value = "127.0.0.1", env = "KND_DATABASE_HOST")]
    pub database_host: String,
    #[clap(long, default_value = "10000", env = "KND_DATABASE_PORT")]
    pub database_port: String,
    #[clap(long, default_value = "root", env = "KND_DATABASE_USER")]
    pub database_user: String,
    #[clap(long, default_value = "defaultdb", env = "KND_DATABASE_NAME")]
    pub database_name: String,
    #[clap(long, default_value = "", env = "KND_DATABASE_PASSWORD")]
    pub database_password: String,
}

impl Settings {
    pub fn load() -> Settings {
        Settings::parse()
    }
}
