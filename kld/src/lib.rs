#![feature(async_closure)]
use async_trait::async_trait;
use tokio::signal::unix::SignalKind;

pub mod api;
pub mod bitcoind;
pub mod database;
pub mod key_generator;
pub mod ldk;
pub mod logger;
pub mod prometheus;
pub mod settings;
pub mod wallet;

// For api codegen
#[macro_use]
extern crate serde;

pub const VERSION: &str = concat!("KLD v", env!("CARGO_PKG_VERSION"));

pub type MillisatAmount = u64;

pub async fn quit_signal() {
    let _ = tokio::signal::unix::signal(SignalKind::quit())
        .unwrap()
        .recv()
        .await;
}

#[async_trait]
pub trait Service: Send + Sync {
    async fn is_connected(&self) -> bool;
    async fn is_synchronised(&self) -> bool;
}

pub fn log_error(e: &anyhow::Error) {
    for cause in e.chain() {
        log::error!("{}", cause);
    }
}
