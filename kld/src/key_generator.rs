#[cfg(not(test))]
use std::fs;
#[cfg(test)]
use test_utils::fake_fs as fs;

use anyhow::{anyhow, Context, Result};
use bdk::keys::bip39::{Language, Mnemonic, WordCount};
use bdk::keys::GeneratableKey;
use bdk::miniscript::Tap;
use bitcoin::hashes::{sha256, Hash, HashEngine};
use log::info;

// To start lets have only one primary seed to backup and derive everything else from that.
pub struct KeyGenerator {
    mnemonic: Mnemonic,
}

impl KeyGenerator {
    pub fn init(mnemonic_path: &str) -> Result<KeyGenerator> {
        let mnemonic = if let Ok(words) = fs::read_to_string(mnemonic_path) {
            info!("Loading mnemonic from {mnemonic_path}");
            Mnemonic::parse(words)?
        } else {
            let mnemonic: bdk::keys::GeneratedKey<_, Tap> =
                Mnemonic::generate((WordCount::Words24, Language::English))
                    .map_err(|_| anyhow!("Mnemonic generation error"))?;

            fs::write(mnemonic_path, mnemonic.to_string())
                .with_context(|| format!("Cannot write to {mnemonic_path}"))?;
            info!("Generated a new mnemonic: {}", mnemonic_path);
            mnemonic.into_key()
        };
        Ok(KeyGenerator { mnemonic })
    }

    pub fn wallet_seed(&self) -> [u8; 32] {
        // The seed can be loaded into any regular wallet and the on chain funds will be available.
        self.generate_key("")
    }

    pub fn lightning_seed(&self) -> [u8; 32] {
        self.generate_key("lightning/0")
    }

    pub fn macaroon_seed(&self) -> [u8; 32] {
        self.generate_key("macaroon/0")
    }

    pub fn promise_seed(&self) -> [u8; 32] {
        self.generate_key("promise_seed")
    }

    fn generate_key(&self, extra_input: &str) -> [u8; 32] {
        let mut engine = sha256::HashEngine::default();
        engine.input(&self.mnemonic.to_seed(""));
        engine.input(extra_input.as_bytes());
        let hash = sha256::Hash::from_engine(engine);
        hash.to_byte_array()
    }
}

#[test]
fn test_key_generator() -> Result<()> {
    let key_generator = KeyGenerator::init("")?;
    let wallet_seed = key_generator.wallet_seed();
    let lightning_seed = key_generator.lightning_seed();
    let macaroon_seed = key_generator.macaroon_seed();

    assert_eq!(wallet_seed, key_generator.wallet_seed());
    assert_eq!(lightning_seed, key_generator.lightning_seed());
    assert_eq!(macaroon_seed, key_generator.macaroon_seed());

    assert_ne!(wallet_seed, lightning_seed);
    assert_ne!(lightning_seed, macaroon_seed);
    Ok(())
}
