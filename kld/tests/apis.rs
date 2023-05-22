use tokio::signal::unix::SignalKind;

mod api;
mod mocks;

pub async fn quit_signal() {
    let _ = tokio::signal::unix::signal(SignalKind::quit())
        .unwrap()
        .recv()
        .await;
}
