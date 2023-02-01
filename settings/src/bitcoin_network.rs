use std::{fmt, str::FromStr};

// Implement me the traits needed for clap to deserialize the following struct:

/// Blockchain to use
#[derive(Copy, PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum Network {
    /// Classic Bitcoin
    Main,
    /// Bitcoin's testnet
    Testnet,
    /// Bitcoin's signet
    Signet,
    /// Bitcoin's regtest
    Regtest,
}

impl fmt::Display for Network {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "{}",
            match self {
                Network::Main => "main",
                Network::Testnet => "testnet",
                Network::Signet => "signet",
                Network::Regtest => "regtest",
            }
        )
    }
}

impl From<Network> for bitcoin::Network {
    fn from(network: Network) -> Self {
        match network {
            Network::Main => bitcoin::Network::Bitcoin,
            Network::Testnet => bitcoin::Network::Testnet,
            Network::Signet => bitcoin::Network::Signet,
            Network::Regtest => bitcoin::Network::Regtest,
        }
    }
}

impl FromStr for Network {
    type Err = &'static str;

    fn from_str(input: &str) -> Result<Network, Self::Err> {
        match input {
            "main" => Ok(Network::Main),
            "testnet" => Ok(Network::Testnet),
            "signet" => Ok(Network::Signet),
            "regtest" => Ok(Network::Regtest),
            _ => Err("not a valid value, must be one of: main, testnet, signet or regtest"),
        }
    }
}
