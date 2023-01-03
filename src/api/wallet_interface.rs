use anyhow::Result;
use bdk::Balance;

pub trait WalletInterface {
    fn balance(&self) -> Result<Balance>;
}
