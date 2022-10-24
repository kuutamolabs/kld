pub mod bitcoind_client;
mod controller;
mod convert;
mod disk;
mod event_handler;
mod hex_utils;
mod logger;
mod net_utils;
mod payment_info;
mod prometheus;
mod settings;

use crate::controller::Controller;
use crate::prometheus::spawn_prometheus_exporter;
use anyhow::Result;
use clap::Parser;
use log::{info, warn};
use settings::Settings;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal::unix::SignalKind;

pub fn main() -> Result<()> {
    crate::logger::init("node_one")?;

    info!("Starting Lightning Kuutamo Node Distribution");

    let settings = Settings::parse();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .build()?;

    let shutdown_flag = Arc::new(AtomicBool::new(false));

    let (controller, background_processor) =
        runtime.block_on(Controller::start_ldk(&settings, shutdown_flag.clone()))?;
    let controller = Arc::new(controller);

    runtime.block_on(async {
        let mut quit_signal = tokio::signal::unix::signal(SignalKind::quit()).unwrap();
        tokio::select!(
            _ = quit_signal.recv() => {
                info!("Received quit signal.");
                Ok(())
            },
            res = spawn_prometheus_exporter(&settings, controller.clone()) => {
                if let Err(e) = res {
                    warn!("Prometheus exporter failed: {}", e);
                    return Err(e);
                }
                res
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
