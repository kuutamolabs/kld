use bitcoin::{secp256k1::PublicKey, Network};
use lightning::{ln::channelmanager::ChannelDetails, routing::gossip};

pub trait LightningInterface {
    fn alias(&self) -> String;

    fn block_height(&self) -> usize;

    fn identity_pubkey(&self) -> PublicKey;

    fn network(&self) -> Network;

    fn num_active_channels(&self) -> usize;

    fn num_inactive_channels(&self) -> usize;

    fn num_pending_channels(&self) -> usize;

    fn graph_num_nodes(&self) -> usize;

    fn graph_num_channels(&self) -> usize;

    fn num_peers(&self) -> usize;

    fn wallet_balance(&self) -> u64;

    fn version(&self) -> String;

    fn list_channels(&self) -> Vec<ChannelDetails>;

    fn get_node(&self, node_id: PublicKey) -> Option<gossip::NodeInfo>;
}
