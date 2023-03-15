use tokio::signal::unix::SignalKind;

pub mod api;
pub mod bitcoind;
pub mod key_generator;
pub mod ldk;
pub mod prometheus;
pub mod wallet;

pub const VERSION: &str = concat!("KLD v", env!("CARGO_PKG_VERSION"));

pub async fn quit_signal() {
    let _ = tokio::signal::unix::signal(SignalKind::quit())
        .unwrap()
        .recv()
        .await;
}
