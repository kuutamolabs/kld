use anyhow::{Context, Result};
use database::ldk_database::LdkDatabase;
use database::migrate_database;
use database::wallet_database::WalletDatabase;
use futures::FutureExt;
use lightning_knd::api::{bind_api_server, MacaroonAuth};
use lightning_knd::bitcoind::BitcoindClient;
use lightning_knd::controller::Controller;
use lightning_knd::key_generator::KeyGenerator;
use lightning_knd::prometheus::start_prometheus_exporter;
use lightning_knd::quit_signal;
use lightning_knd::wallet::Wallet;
use log::info;
use settings::Settings;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub fn main() -> Result<()> {
    let settings = Arc::new(Settings::load());
    logger::KndLogger::init(&settings.node_id, settings.log_level.parse().unwrap());

    info!("Starting Lightning Kuutamo Node Distribution");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .build()?;

    let _g = runtime.enter();

    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let quit_signal = quit_signal().shared();

    runtime.block_on(migrate_database(&settings))?;

    let key_generator = Arc::new(
        KeyGenerator::init(&settings.mnemonic_path).context("cannot initialize key generator")?,
    );

    let database = Arc::new(
        runtime
            .block_on(LdkDatabase::new(&settings))
            .context("cannot connect to ldk database")?,
    );
    let wallet_database = runtime
        .block_on(WalletDatabase::new(&settings))
        .context("cannot connect to wallet database")?;

    let bitcoind_client = Arc::new(runtime.block_on(BitcoindClient::new(settings.as_ref()))?);
    runtime.block_on(bitcoind_client.wait_for_blockchain_synchronisation())?;
    bitcoind_client.poll_for_fee_estimates();

    let wallet = Arc::new(
        Wallet::new(
            &key_generator.wallet_seed(),
            &settings,
            bitcoind_client.clone(),
            wallet_database,
        )
        .context("Cannot create wallet")?,
    );

    let (controller, background_processor) = runtime
        .block_on(Controller::start_ldk(
            settings.clone(),
            database,
            bitcoind_client,
            wallet.clone(),
            key_generator.clone(),
        ))
        .context("Failed to start ldk controller")?;
    let controller = Arc::new(controller);

    let macaroon_auth = Arc::new(MacaroonAuth::init(
        &key_generator.macaroon_seed(),
        &settings.data_dir,
    )?);

    runtime.block_on(async {
        let server = bind_api_server(settings.rest_api_address.clone(), settings.certs_dir.clone()).await?;

        tokio::select!(
            _ = quit_signal.clone() => {
                info!("Received quit signal.");
                Ok(())
            },
            result = start_prometheus_exporter(settings.exporter_address.clone(), controller.clone(), quit_signal.clone()) => {
                result.context("Prometheus exporter failed")
            },
            result = server.serve(controller.clone(), wallet.clone(), macaroon_auth, quit_signal) => {
                result.context("REST API failed")
            }
        )
    })?;

    info!("Shutting down");
    shutdown_flag.store(true, Ordering::Release);
    let res = background_processor
        .stop()
        .context("could not stop background processor");
    controller.stop();
    res?;
    runtime.shutdown_timeout(Duration::from_secs(30));
    info!("Stopped all threads. Process finished.");
    Ok(())
}
