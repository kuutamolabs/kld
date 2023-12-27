pub mod details;
pub mod list;
pub mod query;
use kld::api::routes;
use serde::Serialize;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize)]
pub enum Cmd {
    AppInfo,
    ChanBala,
    ChanClos,
    ChanHist,
    ChanList,
    ChanLsfd,
    ChanOpen,
    ChanSetf,
    InvoDeco,
    InvoGene,
    InvoList,
    NetwFeer,
    NetwLsnd,
    NodeEslq,
    NodeFees,
    NodeInfo,
    NodeLsfd,
    NodeSign,
    PaymList,
    PaymPayi,
    PaymSdky,
    PeerCont,
    PeerDisc,
    PeerList,
}

impl Cmd {
    pub fn get_uri(&self) -> Option<&'static str> {
        match self {
            Cmd::ChanBala => Some(routes::LOCAL_REMOTE_BALANCE),
            Cmd::ChanClos => Some(routes::CLOSE_CHANNEL),
            Cmd::ChanHist => Some(routes::LIST_CHANNEL_HISTORY),
            Cmd::ChanList => Some(routes::LIST_CHANNELS),
            Cmd::ChanLsfd => Some(routes::LIST_FORWARDS),
            Cmd::ChanOpen => Some(routes::OPEN_CHANNEL),
            Cmd::ChanSetf => Some(routes::SET_CHANNEL_FEE),
            Cmd::InvoDeco => Some(routes::DECODE_INVOICE),
            Cmd::InvoGene => Some(routes::GENERATE_INVOICE),
            Cmd::InvoList => Some(routes::LIST_INVOICES),
            Cmd::NetwFeer => Some(routes::FEE_RATES),
            Cmd::NetwLsnd => Some(routes::LIST_NETWORK_NODES),
            Cmd::NodeEslq => Some(routes::ESTIMATE_CHANNEL_LIQUIDITY),
            Cmd::NodeFees => Some(routes::GET_FEES),
            Cmd::NodeInfo => Some(routes::GET_INFO),
            Cmd::NodeLsfd => Some(routes::LIST_FUNDS),
            Cmd::NodeSign => Some(routes::SIGN),
            Cmd::PaymList => Some(routes::LIST_PAYMENTS),
            Cmd::PaymPayi => Some(routes::PAY_INVOICE),
            Cmd::PaymSdky => Some(routes::KEYSEND),
            Cmd::PeerCont => Some(routes::CONNECT_PEER),
            Cmd::PeerDisc => Some(routes::DISCONNECT_PEER),
            Cmd::PeerList => Some(routes::LIST_PEERS),
            _ => None,
        }
    }
}
