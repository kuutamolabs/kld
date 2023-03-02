use tokio::signal::unix::SignalKind;

pub mod api;
pub mod bitcoind;
pub mod controller;
mod event_handler;
pub mod key_generator;
pub mod net_utils;
mod payment_info;
mod peer_manager;
pub mod prometheus;
pub mod wallet;

pub const VERSION: &str = concat!("Lightning KND v", env!("CARGO_PKG_VERSION"));

pub async fn quit_signal() {
    let _ = tokio::signal::unix::signal(SignalKind::quit())
        .unwrap()
        .recv()
        .await;
}
