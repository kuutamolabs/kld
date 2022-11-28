mod cipher;
pub mod ldk_database;
pub mod peer;
pub mod wallet_database;

use anyhow::Result;
use log::error;
use settings::Settings;
pub use tokio_postgres::{Client, NoTls, Transaction};

#[macro_export]
macro_rules! to_i64 {
    ($int: expr) => {
        i64::try_from($int).unwrap()
    };
}

#[macro_export]
macro_rules! from_i64 {
    ($row: expr, $name: expr) => {
        $row.get::<&str, i64>(&$name).try_into().unwrap()
    };
}

#[macro_export]
macro_rules! from_maybe_i64 {
    ($row: expr, $name: expr) => {
        $row.get::<&str, Option<i64>>(&$name)
            .map(|x| x.try_into().unwrap())
    };
}

async fn connection(settings: &Settings) -> Result<Client> {
    let mut params = format!(
        "host={} port={} user={} dbname={}",
        settings.database_host,
        settings.database_port,
        settings.database_user,
        settings.database_name
    );
    if !settings.database_password.is_empty() {
        params = format!("{} password={}", params, settings.database_password);
    }
    let (client, connection) = tokio_postgres::connect(&params, NoTls).await?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            error!("Database connection error: {}", e);
        }
    });
    Ok(client)
}

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("sql");
}

pub async fn migrate_database(settings: &Settings) -> Result<()> {
    let mut client = connection(&settings).await?;
    embedded::migrations::runner()
        .run_async(&mut client)
        .await?;
    Ok(())
}
