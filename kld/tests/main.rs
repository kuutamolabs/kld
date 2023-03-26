use settings::Settings;
use tokio::signal::unix::SignalKind;

mod database;
mod integration;
mod mocks;
mod smoke;

pub async fn quit_signal() {
    let _ = tokio::signal::unix::signal(SignalKind::quit())
        .unwrap()
        .recv()
        .await;
}

pub fn test_settings(name: &str) -> Settings {
    test_utils::test_settings(env!("CARGO_TARGET_TMPDIR"), name)
}
