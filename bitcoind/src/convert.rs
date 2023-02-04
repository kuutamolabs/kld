use bitcoin::hashes::hex::FromHex;
use bitcoin::BlockHash;
use lightning_block_sync::http::JsonResponse;
use std::convert::TryInto;

pub struct RawTx(pub String);

impl TryInto<RawTx> for JsonResponse {
    type Error = std::io::Error;
    fn try_into(self) -> std::io::Result<RawTx> {
        Ok(RawTx(self.0.as_str().unwrap().to_string()))
    }
}

pub struct NewAddress(pub String);
impl TryInto<NewAddress> for JsonResponse {
    type Error = std::io::Error;
    fn try_into(self) -> std::io::Result<NewAddress> {
        Ok(NewAddress(self.0.as_str().unwrap().to_string()))
    }
}

pub struct FeeResponse {
    pub feerate_sat_per_kw: Option<u32>,
    pub errored: bool,
}

impl TryInto<FeeResponse> for JsonResponse {
    type Error = std::io::Error;
    fn try_into(self) -> std::io::Result<FeeResponse> {
        let errored = !self.0["errors"].is_null();
        Ok(FeeResponse {
            errored,
            feerate_sat_per_kw: self.0["feerate"].as_f64().map(|feerate_btc_per_kvbyte| {
                // Bitcoin Core gives us a feerate in BTC/KvB, which we need to convert to
                // satoshis/KW. Thus, we first multiply by 10^8 to get satoshis, then divide by 4
                // to convert virtual-bytes into weight units.
                (feerate_btc_per_kvbyte * 100_000_000.0 / 4.0).round() as u32
            }),
        })
    }
}

pub struct BlockchainInfo {
    pub blocks: usize,
    pub headers: usize,
    pub best_block_hash: BlockHash,
    pub verification_progress: f64,
    pub mediantime: usize,
    pub chain: String,
}

impl TryInto<BlockchainInfo> for JsonResponse {
    type Error = std::io::Error;
    fn try_into(self) -> std::io::Result<BlockchainInfo> {
        Ok(BlockchainInfo {
            blocks: self.0["blocks"].as_u64().unwrap() as usize,
            headers: self.0["headers"].as_u64().unwrap() as usize,
            mediantime: self.0["mediantime"].as_u64().unwrap() as usize,
            best_block_hash: BlockHash::from_hex(self.0["bestblockhash"].as_str().unwrap())
                .unwrap(),
            verification_progress: self.0["verificationprogress"].as_f64().unwrap(),
            chain: self.0["chain"].as_str().unwrap().to_string(),
        })
    }
}
