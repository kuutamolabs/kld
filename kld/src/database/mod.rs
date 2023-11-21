pub mod channel;
pub mod forward;
pub mod invoice;
mod ldk_database;
pub mod payment;
pub mod peer;
pub mod spendable_output;
mod wallet_database;

use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use async_trait::async_trait;
pub use ldk_database::LdkDatabase;
use lightning::util::ser::MaybeReadable;
use postgres_types::ToSql;
use time::{OffsetDateTime, PrimitiveDateTime};
use tokio::{sync::OwnedRwLockReadGuard, task::JoinHandle};
pub use wallet_database::WalletDatabase;

use anyhow::{Context, Result};
use log::{error, info};
use openssl::ssl::{SslConnector, SslFiletype, SslMethod};
use postgres_openssl::MakeTlsConnector;
use tokio::sync::RwLock as AsyncRwLock;
use tokio_postgres::{Client, Row};

use crate::{ldk::decode_error, settings::Settings};

use crate::{log_error, Service};

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

pub struct DurableConnection {
    client: Arc<AsyncRwLock<Client>>, // Used across await points.
    connection_task: Arc<RwLock<JoinHandle<()>>>,
    reconnect_task: JoinHandle<()>,
}

#[async_trait]
impl Service for DurableConnection {
    async fn is_connected(&self) -> bool {
        !self.get().await.is_closed()
    }

    async fn is_synchronised(&self) -> bool {
        true
    }
}

#[async_trait]
pub trait DBConnection: Service {
    async fn open_channel_count(&self) -> Result<i64>;
    async fn fetch_scorer(&self) -> Result<Vec<u8>>;
    async fn fetch_scorer_update_time(&self) -> Result<OffsetDateTime>;
}

#[async_trait]
impl DBConnection for DurableConnection {
    async fn open_channel_count(&self) -> Result<i64> {
        let row = self
            .get()
            .await
            .query_one(
                "SELECT COUNT(*) FROM channels WHERE close_timestamp IS NULL;",
                &[],
            )
            .await?;
        let count: i64 = row.get("count");
        Ok(count)
    }
    async fn fetch_scorer(&self) -> Result<Vec<u8>> {
        let row = self
            .get()
            .await
            .query_one("SELECT scorer FROM scorer;", &[])
            .await?;
        Ok(row.get("scorer"))
    }

    async fn fetch_scorer_update_time(&self) -> Result<OffsetDateTime> {
        let row = self
            .get()
            .await
            .query_one("SELECT timestamp FROM scorer;", &[])
            .await?;
        Ok(row.get_timestamp("timestamp"))
    }
}

impl DurableConnection {
    pub async fn new_migrate(settings: Arc<Settings>) -> DurableConnection {
        info!(
            "Connecting to Cockroach database {} at {}:{}",
            settings.database_name, settings.database_host, settings.database_port
        );
        // The service cannot start properly without the database so we wait here.
        let (mut client, connection_task) = loop {
            match DurableConnection::create_connection(settings.clone()).await {
                Ok(client) => break client,
                Err(e) => {
                    log_error(&e);
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        };
        info!("Running database migrations for {}", settings.database_name);
        embedded::migrations::runner()
            .run_async(&mut client)
            .await
            .expect("failed to run migrations");

        let client = Arc::new(AsyncRwLock::new(client));
        let connection_task = Arc::new(RwLock::new(connection_task));
        let reconnect_task = DurableConnection::keep_connected(
            settings.clone(),
            client.clone(),
            connection_task.clone(),
        );
        DurableConnection {
            client,
            connection_task,
            reconnect_task,
        }
    }

    pub fn disconnect(&self) {
        info!("Disconnecting from database");
        self.reconnect_task.abort();
        match self.connection_task.write() {
            Ok(guard) => guard.abort(),
            Err(e) => error!("{e}"),
        }
    }

    async fn create_connection(settings: Arc<Settings>) -> Result<(Client, JoinHandle<()>)> {
        let log_safe_params = format!(
            "host={} port={} user={} dbname={}",
            settings.database_host,
            settings.database_port,
            settings.database_user,
            settings.database_name
        );
        let mut builder = SslConnector::builder(SslMethod::tls()).expect("TLS initialisation");
        builder.set_ca_file(&settings.database_ca_cert_path)?;
        builder
            .set_certificate_file(&settings.database_client_cert_path, SslFiletype::PEM)
            .expect("Database certificate");
        builder
            .set_private_key_file(&settings.database_client_key_path, SslFiletype::PEM)
            .expect("Database private key");
        let connector = MakeTlsConnector::new(builder.build());
        let (client, connection) = tokio_postgres::connect(&log_safe_params, connector)
            .await
            .with_context(|| format!("Cannot connect to database ({log_safe_params})"))?;
        let connection_task = tokio::spawn(async move {
            if let Err(e) = connection.await {
                error!("{e}");
            }
        });
        Ok((client, connection_task))
    }

    // Get the current connection no matter what state it is in (may error when used).
    async fn get(&self) -> OwnedRwLockReadGuard<Client> {
        self.client.clone().read_owned().await
    }

    /// Block on trying to reconnect to the database if the connection has been dropped.
    /// This can probably only be used during start up when we have to wait. Take care not to block async tasks.
    async fn wait(&self) -> OwnedRwLockReadGuard<Client> {
        loop {
            let client = self.get().await;
            if !client.is_closed() {
                return client;
            } else {
                drop(client);
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }

    fn keep_connected(
        settings: Arc<Settings>,
        client: Arc<AsyncRwLock<Client>>,
        connection_task: Arc<RwLock<JoinHandle<()>>>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                if client.read().await.is_closed() {
                    let mut client_guard = client.write().await;
                    match DurableConnection::create_connection(settings.clone()).await {
                        Ok((client, connect_task)) => {
                            *client_guard = client;
                            match connection_task.write() {
                                Ok(mut task_guard) => *task_guard = connect_task,
                                Err(e) => error!("{e}"),
                            }
                        }
                        Err(e) => {
                            error!("{e}");
                        }
                    }
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        })
    }
}

impl Drop for DurableConnection {
    fn drop(&mut self) {
        self.disconnect()
    }
}

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("src/database/sql");
}

#[derive(Default)]
pub struct Params<'a> {
    vec: Vec<Box<(dyn ToSql + Sync + Send + 'a)>>,
}

impl<'a> Params<'a> {
    pub fn push(&mut self, x: impl ToSql + Sync + Send + 'a) {
        self.vec.push(Box::new(x))
    }

    pub fn count(&self) -> usize {
        self.vec.len()
    }

    pub fn to_params(&self) -> Vec<&(dyn ToSql + Sync)> {
        self.vec
            .iter()
            .map(|x| x.as_ref() as &(dyn ToSql + Sync))
            .collect()
    }
}

pub fn microsecond_timestamp() -> OffsetDateTime {
    let timestamp = OffsetDateTime::now_utc();
    timestamp
        .replace_nanosecond(timestamp.microsecond() * 1000)
        .unwrap()
}

pub fn to_primitive(offset: &OffsetDateTime) -> PrimitiveDateTime {
    PrimitiveDateTime::new(offset.date(), offset.time())
}

pub trait RowExt {
    fn get_timestamp(&self, column: &str) -> OffsetDateTime;
    fn get_timestamp_optional(&self, column: &str) -> Option<OffsetDateTime>;
    fn read<T: MaybeReadable>(&self, column: &str) -> Result<T>;
    fn read_optional<T: MaybeReadable>(&self, column: &str) -> Result<Option<T>>;
}

impl RowExt for Row {
    fn read<T: MaybeReadable>(&self, column: &str) -> Result<T> {
        T::read(&mut self.get::<&str, &[u8]>(column))
            .map_err(decode_error)?
            .context(format!("expected readable value for column {column}"))
    }

    fn read_optional<T: MaybeReadable>(&self, column: &str) -> Result<Option<T>> {
        self.get::<&str, Option<&[u8]>>(column)
            .map(|mut bytes| {
                T::read(&mut bytes)
                    .map_err(decode_error)?
                    .context(format!("expected readable value for column {column}"))
            })
            .transpose()
    }

    fn get_timestamp(&self, column: &str) -> OffsetDateTime {
        self.get::<&str, PrimitiveDateTime>(column).assume_utc()
    }

    fn get_timestamp_optional(&self, column: &str) -> Option<OffsetDateTime> {
        self.get::<&str, Option<PrimitiveDateTime>>(column)
            .map(|time| time.assume_utc())
    }
}
