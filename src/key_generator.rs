#[cfg(not(test))]
use std::fs;
#[cfg(test)]
use test_utils::fake_fs as fs;

use anyhow::{bail, Context, Result};
use bitcoin::hashes::{sha256, Hash, HashEngine};
use log::info;
use rand::{thread_rng, Rng};

// To start lets have only one primary seed to backup and derive everything else from that.
pub struct KeyGenerator {
    seed: [u8; 32],
}

impl KeyGenerator {
    pub fn init(data_dir: &str) -> Result<KeyGenerator> {
        let seed_path = format!("{}/secret_seed", data_dir);
        let seed = if let Ok(seed) = fs::read(&seed_path) {
            info!("Loading secret seed: {}", seed_path);
            match seed.try_into() {
                Err(_) => bail!("Invalid seed file at {}", seed_path),
                Ok(v) => v,
            }
        } else {
            let seed: [u8; 32] = thread_rng().gen();
            fs::write(&seed_path, seed).with_context(|| format!("cannot write {}", seed_path))?;
            info!("Generated secret seed: {}", seed_path);
            seed
        };
        Ok(KeyGenerator { seed })
    }

    pub fn wallet_seed(&self) -> [u8; 32] {
        self.generate_key("wallet/0")
    }

    pub fn lightning_seed(&self) -> [u8; 32] {
        self.generate_key("lightning/0")
    }

    pub fn macaroon_seed(&self) -> [u8; 32] {
        self.generate_key("macaroon/0")
    }

    fn generate_key(&self, extra_input: &str) -> [u8; 32] {
        let mut engine = sha256::HashEngine::default();
        engine.input(&self.seed);
        engine.input(extra_input.as_bytes());
        let hash = sha256::Hash::from_engine(engine);
        hash.into_inner()
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
