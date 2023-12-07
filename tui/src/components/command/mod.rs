pub mod details;
pub mod list;
pub mod query;
use serde::Serialize;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize)]
pub enum Cmd {
    AppInfo,
    NodeInfo,
    NodeFees,
    NodeEslq,
    NodeSign,
    NodeLsfd,
    NetwLsnd,
    NetwFeer,
    PeerList,
    PeerCont,
    PeerDisc,
    PaymList,
    PaymSdky,
    PaymPayi,
    InvoList,
    InvoGene,
    InvoDeco,
    ChanList,
    ChanOpen,
    ChanSetf,
    ChanClos,
    ChanHist,
    ChanBala,
    ChanLsfd,
}

impl Cmd {
    pub fn get_uri(&self) -> Option<&'static str> {
        match self {
            Cmd::NodeInfo => Some("/v1/getinfo"),
            Cmd::PeerCont => Some("/v1/peer/connect"),
            _ => None,
        }
    }
}
