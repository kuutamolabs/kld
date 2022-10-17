use bitcoin::Network;
use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about, long_about = None)]
pub struct Settings {
    #[clap(long, default_value = "localhost", env = "BITCOIN_RPC_HOST")]
    pub bitcoind_rpc_host: String,
    #[clap(long, default_value = "8333", env = "BITCOIN_RPC_PORT")]
    pub bitcoind_rpc_port: u16,
    #[clap(long, default_value = "testnet", env = "BITCOIN_NETWORK")]
    pub bitcoin_network: Network,
    #[clap(long, default_value = "testnet", env = "BITCOIN_COOKIE_PATH")]
    pub bitcoin_cookie_path: String,

    #[clap(long, default_value = ".", env = "KND_STORAGE_DIR")]
    pub knd_storage_dir: String,
    #[clap(long, default_value = "9234", env = "KND_PEER_PORT")]
    pub knd_peer_port: String,
    #[clap(long, default_value = "testnode", env = "KND_NODE_NAME")]
    pub knd_node_name: String,
    #[clap(long, default_value = "127.0.0.1:9234", env = "KND_LISTEN_ADDR")]
    pub knd_listen_addr: Vec<String>,
}
