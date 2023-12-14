use anyhow::{Context, Result};
use futures::FutureExt;
use kld::api::{bind_api_server, MacaroonAuth};
use kld::bitcoind::BitcoindClient;
use kld::database::{DurableConnection, WalletDatabase};
use kld::key_generator::KeyGenerator;
use kld::ldk::Controller;
use kld::logger::KldLogger;
use kld::prometheus::start_prometheus_exporter;
use kld::settings::Settings;
use kld::wallet::Wallet;
use kld::{log_error, quit_signal, VERSION};
use log::{error, info};
use prometheus::IntCounter;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

static PROBE_TOTAL_COUNT: OnceLock<IntCounter> = OnceLock::new();
static PROBE_SUCCESSFUL_COUNT: OnceLock<IntCounter> = OnceLock::new();
static PROBE_FAILED_COUNT: OnceLock<IntCounter> = OnceLock::new();

pub fn main() {
    let settings = Arc::new(Settings::load());
    KldLogger::init(
        &settings.node_id,
        settings.log_level.parse().expect("Invalid log level"),
    );

    info!("Starting {VERSION}");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .build()
        .expect("could not create runtime");

    let exit_code = if let Err(e) = runtime.block_on(run_kld(settings)) {
        error!("Fatal error encountered: {e}");
        log_error(&e);
        error!("{}", e.backtrace());
        1
    } else {
        0
    };

    info!("Shutting down");
    runtime.shutdown_timeout(Duration::from_secs(30));
    info!("Stopped all threads. Process finished.");
    std::process::exit(exit_code);
}

async fn run_kld(settings: Arc<Settings>) -> Result<()> {
    let quit_signal = quit_signal().shared();

    let durable_connection = Arc::new(DurableConnection::new_migrate(settings.clone()).await);

    let key_generator = Arc::new(
        KeyGenerator::init(&settings.mnemonic_path).context("cannot initialize key generator")?,
    );

    let wallet_database = WalletDatabase::new(settings.clone(), durable_connection.clone());

    let bitcoind_client = Arc::new(BitcoindClient::new(&settings).await?);
    bitcoind_client.poll_for_fee_estimates();

    let wallet = Arc::new(
        Wallet::new(
            &key_generator.wallet_seed(),
            settings.clone(),
            bitcoind_client.clone(),
            wallet_database,
        )
        .context("Cannot create wallet")?,
    );

    let controller = Controller::start_ldk(
        settings.clone(),
        durable_connection.clone(),
        bitcoind_client.clone(),
        wallet.clone(),
        &key_generator.lightning_seed(),
        quit_signal.clone(),
        (
            &PROBE_TOTAL_COUNT,
            &PROBE_SUCCESSFUL_COUNT,
            &PROBE_FAILED_COUNT,
        ),
    )
    .await
    .context("Failed to start ldk controller")?;
    let controller = Arc::new(controller);

    let macaroon_auth = Arc::new(MacaroonAuth::init(
        &key_generator.macaroon_seed(),
        &settings.data_dir,
    )?);

    let server = bind_api_server(
        settings.rest_api_address.clone(),
        settings.certs_dir.clone(),
    )
    .await?;

    tokio::select!(
        _ = quit_signal.clone() => {
            info!("Received quit signal. Will shtudwon after {} seconds graceful period", settings.shutdown_graceful_sec);
            tokio::time::sleep(Duration::from_secs(settings.shutdown_graceful_sec)).await;
            info!("Force shutdown");
            Ok(())
        },
        result = start_prometheus_exporter(settings.exporter_address.clone(), controller.clone(), durable_connection.clone(), bitcoind_client.clone(), quit_signal.clone(),
                                           (&PROBE_TOTAL_COUNT, &PROBE_SUCCESSFUL_COUNT, &PROBE_FAILED_COUNT)) => {
            result.context("Prometheus exporter failed")
        },
        result = server.serve(bitcoind_client.clone(), controller.clone(), wallet.clone(), macaroon_auth, quit_signal) => {
            result.context("REST API failed")
        }
    )
}
