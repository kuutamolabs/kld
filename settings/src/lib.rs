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

    #[clap(long, default_value = "info", env = "KND_LOG_LEVEL")]
    pub log_level: String,
    #[clap(long, default_value = "test", env = "KND_ENV")]
    pub env: String,
    #[clap(long, default_value = ".", env = "KND_STORAGE_DIR")]
    pub knd_storage_dir: String,
    #[clap(long, default_value = "9234", env = "KND_PEER_PORT")]
    pub knd_peer_port: u16,
    #[clap(long, default_value = "testnode", env = "KND_NODE_NAME")]
    pub knd_node_name: String,
    #[clap(long, default_value = "127.0.0.1:9234", env = "KND_LISTEN_ADDRESSES")]
    pub knd_listen_addresses: Vec<String>,

    #[clap(long, default_value = "127.0.0.1:2233", env = "KND_EXPORTER_ADDRESS")]
    pub exporter_address: String,

    #[clap(long, default_value = "local", env = "KND_S3_REGION")]
    pub s3_region: String,
    #[clap(long, default_value = "127.0.0.1:9000", env = "KND_S3_ADDRESS")]
    pub s3_address: String,
    #[clap(long, default_value = "minioadmin", env = "KND_S3_ACCESS_KEY")]
    pub s3_access_key: String,
    #[clap(long, default_value = "minioadmin", env = "KND_S3_SECRET_KEY")]
    pub s3_secret_key: String,
    #[clap(
        long,
        default_value = "00000000000000000000000000000000",
        env = "KND_S3_ENCRYPTION_KEY"
    )]
    pub s3_encryption_key: String,
}

impl Settings {
    pub fn load() -> Settings {
        Settings::parse()
    }
}
