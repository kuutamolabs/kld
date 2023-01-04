use tokio::signal::unix::SignalKind;

pub mod api;
mod mock_lightning;
mod mock_wallet;
pub mod prometheus;

pub async fn quit_signal() {
    let _ = tokio::signal::unix::signal(SignalKind::quit())
        .unwrap()
        .recv()
        .await;
}
