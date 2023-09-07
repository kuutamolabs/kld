use anyhow::Result;
use kld::database::DurableConnection;
use kld::settings::Settings;
use tempfile::TempDir;
use test_utils::cockroach_manager::create_database;
use test_utils::test_settings;
use test_utils::CockroachManager;

mod ldk_database;
mod wallet_database;

// XXX
// Why do we make a crate `test-utils`?
// Test utils should be collect.
pub async fn init_db_test_context(
    temp_dir: &TempDir,
) -> Result<(Settings, CockroachManager, DurableConnection)> {
    let mut settings = test_settings(temp_dir, "integration");
    let cockroach = CockroachManager::builder(temp_dir, &mut settings)
        .await?
        .build()
        .await?;
    create_database(&settings).await;
    let durable_connection = DurableConnection::new_migrate(settings.clone().into()).await;
    Ok((settings, cockroach, durable_connection))
}
