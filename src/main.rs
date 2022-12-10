mod controller;
mod event_handler;
mod hex_utils;
mod net_utils;
mod payment_info;
mod prometheus;
mod wallet;

use crate::controller::Controller;
use crate::prometheus::start_prometheus_exporter;
use anyhow::Result;
use api::start_rest_api;
use database::ldk_database::LdkDatabase;
use database::migrate_database;
use futures::FutureExt;
use log::{info, warn};
use settings::Settings;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal::unix::SignalKind;

pub const VERSION: &str = concat!("LYND v", env!("CARGO_PKG_VERSION"));

pub fn main() -> Result<()> {
    let settings = Arc::new(Settings::load());
    logger::KndLogger::init(&settings.node_id, settings.log_level.parse().unwrap());

    info!("Starting Lightning Kuutamo Node Distribution");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .build()?;

    let shutdown_flag = Arc::new(AtomicBool::new(false));

    runtime.block_on(migrate_database(&settings))?;

    let database = Arc::new(runtime.block_on(LdkDatabase::new(&settings))?);

    let (controller, background_processor) = runtime.block_on(Controller::start_ldk(
        settings.clone(),
        database,
        shutdown_flag.clone(),
    ))?;
    let controller = Arc::new(controller);

    runtime.block_on(async {
        let quit_signal = quit_signal().shared();
        let quit_signal_api = quit_signal.clone();
        tokio::select!(
            _ = quit_signal => {
                info!("Received quit signal.");
                Ok(())
            },
            result = start_prometheus_exporter(settings.exporter_address.clone(), controller.clone()) => {
                if let Err(e) = result {
                    warn!("Prometheus exporter failed: {}", e);
                    return Err(e);
                }
                result
            },
            result = start_rest_api(&settings.rest_api_address, controller.clone(), quit_signal_api) => {
                if let Err(e) = result {
                    warn!("REST API failed: {}", e);
                    return Err(e);
                }
                result
            }
        )
    })?;

    info!("Shutting down");
    shutdown_flag.store(true, Ordering::Release);
    background_processor.stop().unwrap();
    controller.stop();
    runtime.shutdown_timeout(Duration::from_secs(30));
    info!("Stopped all threads. Process finished.");
    Ok(())
}

async fn quit_signal() {
    let _ = tokio::signal::unix::signal(SignalKind::quit())
        .unwrap()
        .recv()
        .await;
}
