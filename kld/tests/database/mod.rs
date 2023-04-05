use std::panic;
use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

use anyhow::Result;
use futures::Future;
use futures::FutureExt;
use kld::database::migrate_database;
use kld::logger::KldLogger;
use once_cell::sync::OnceCell;
use settings::Settings;
use test_utils::cockroach_manager::create_database;
use test_utils::{cockroach, CockroachManager};
use tokio::runtime::Handle;

use crate::test_settings;

mod ldk_database;
mod wallet_database;

static COCKROACH_REF_COUNT: AtomicU16 = AtomicU16::new(0);

pub async fn with_cockroach<F, Fut>(test: F) -> Result<()>
where
    F: FnOnce(&'static Settings) -> Fut,
    Fut: Future<Output = Result<()>>,
{
    let (settings, _cockroach) = cockroach().await?;
    let result = panic::AssertUnwindSafe(test(settings)).catch_unwind().await;

    teardown().await;
    match result {
        Err(e) => panic::resume_unwind(e),
        Ok(v) => v,
    }
}

// Need to call teardown function at the end of the test if using this.
async fn cockroach() -> Result<&'static (Settings, Mutex<CockroachManager>)> {
    COCKROACH_REF_COUNT.fetch_add(1, Ordering::AcqRel);
    static INSTANCE: OnceCell<(Settings, Mutex<CockroachManager>)> = OnceCell::new();
    INSTANCE.get_or_try_init(|| {
        KldLogger::init("test", log::LevelFilter::Debug);
        tokio::task::block_in_place(move || {
            Handle::current().block_on(async move {
                let mut settings = test_settings("integration");
                let cockroach = cockroach!(settings);
                create_database(&settings).await;
                migrate_database(&settings).await;
                Ok((settings, Mutex::new(cockroach)))
            })
        })
    })
}

pub async fn teardown() {
    if COCKROACH_REF_COUNT.fetch_sub(1, Ordering::AcqRel) == 1 {
        if let Ok(c) = cockroach().await {
            let mut lock = c.1.lock().unwrap();
            lock.kill();
        }
    }
}
