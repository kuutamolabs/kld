use anyhow::Result;
use database::ldk_database::LdkDatabase;
use database::migrate_database;
use futures::FutureExt;
use lightning_knd::api::{start_rest_api, MacaroonAuth};
use lightning_knd::controller::Controller;
use lightning_knd::key_generator::KeyGenerator;
use lightning_knd::prometheus::start_prometheus_exporter;
use log::{info, warn};
use settings::Settings;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal::unix::SignalKind;

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

    let key_generator = Arc::new(KeyGenerator::init(&settings.data_dir));

    let database = Arc::new(runtime.block_on(LdkDatabase::new(&settings))?);

    let (controller, background_processor) = runtime.block_on(Controller::start_ldk(
        settings.clone(),
        database,
        key_generator.clone(),
        shutdown_flag.clone(),
    ))?;
    let controller = Arc::new(controller);

    let macaroon_auth = Arc::new(MacaroonAuth::init(
        &key_generator.macaroon_seed(),
        &settings.data_dir,
    )?);

    runtime.block_on(async {
        let quit_signal = quit_signal().shared();
        tokio::select!(
            _ = quit_signal.clone() => {
                info!("Received quit signal.");
                Ok(())
            },
            result = start_prometheus_exporter(settings.exporter_address.clone(), controller.clone(), quit_signal.clone()) => {
                if let Err(e) = result {
                    warn!("Prometheus exporter failed: {}", e);
                    return Err(e);
                }
                result
            },
            result = start_rest_api(settings.rest_api_address.clone(), controller.clone(), macaroon_auth, quit_signal) => {
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
