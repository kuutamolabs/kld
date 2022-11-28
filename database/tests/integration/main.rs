use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering;

use database::migrate_database;
use logger::KndLogger;
use settings::Settings;
use test_utils::cockroach;
use test_utils::test_settings_for_database;
use test_utils::CockroachManager;

pub mod ldk_database;
pub mod wallet_database;

static COUNT: AtomicU16 = AtomicU16::new(0);

pub async fn cockroach_manager() -> (CockroachManager, Settings) {
    let c = COUNT.fetch_add(1, Ordering::SeqCst);

    if c == 0 {
        KndLogger::init("test", "info").unwrap(); // can only be set once per test suite.
    }

    let mut cockroach = cockroach!(c);
    cockroach.start().await;
    let settings = test_settings_for_database(&cockroach);
    let s = settings.clone();
    migrate_database(&s).await.unwrap();
    (cockroach, settings)
}
