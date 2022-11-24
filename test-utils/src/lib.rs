pub mod bitcoin_manager;
pub mod cockroach_manager;
pub mod knd_manager;

use bitcoin::secp256k1::{PublicKey, SecretKey};
use clap::{builder::OsStr, Parser};
use cockroach_manager::CockroachManager;
use settings::Settings;

pub struct TestSettingsBuilder {
    settings: Settings,
}

impl TestSettingsBuilder {
    pub fn new() -> TestSettingsBuilder {
        TestSettingsBuilder {
            settings: Settings::parse_from::<Vec<OsStr>, OsStr>(vec![]),
        }
    }

    pub fn with_node_id(mut self, node_id: &str) -> TestSettingsBuilder {
        self.settings.node_id = node_id.to_string();
        self
    }

    pub fn for_database(mut self, database: &CockroachManager) -> TestSettingsBuilder {
        self.settings.database_port = database.port.to_string();
        self
    }

    pub fn build(self) -> Settings {
        self.settings
    }
}

pub fn test_settings() -> Settings {
    TestSettingsBuilder::new().build()
}

pub fn test_settings_for_database(database: &CockroachManager) -> Settings {
    TestSettingsBuilder::new().for_database(database).build()
}

#[macro_export]
macro_rules! poll {
    ($secs: expr, $func: expr) => {
        let mut ellapsed = 0;
        while ellapsed < $secs {
            if $func {
                break;
            };
            tokio::time::sleep(Duration::from_secs(1)).await;
            ellapsed += 1;
        }
        if ellapsed == $secs {
            panic!("Timed out polling for result");
        }
    };
}

// Use #[unstable(feature = "thread_id_value", issue = "67939")] when its stable.
pub fn unique_number() -> u16 {
    let mut thread_id = format!("{:?}", std::thread::current().id());
    thread_id.retain(|c| ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'].contains(&c));
    thread_id.parse::<u64>().unwrap() as u16
}

pub fn random_public_key() -> PublicKey {
    let rand: [u8; 32] = rand::random();
    let secp_ctx = bitcoin::secp256k1::Secp256k1::new();
    let secret_key = &SecretKey::from_slice(&rand).unwrap();
    PublicKey::from_secret_key(&secp_ctx, secret_key)
}
