use anyhow::{Context, Result};
use futures::FutureExt;
use kld::api::{bind_api_server, MacaroonAuth};
use kld::bitcoind::BitcoindClient;
use kld::database::{migrate_database, WalletDatabase};
use kld::key_generator::KeyGenerator;
use kld::ldk::Controller;
use kld::logger::KldLogger;
use kld::prometheus::start_prometheus_exporter;
use kld::wallet::Wallet;
use kld::{quit_signal, VERSION};
use log::{error, info};
use settings::Settings;
use std::sync::Arc;
use std::time::Duration;

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
        for cause in e.chain() {
            error!("{}", cause);
        }
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

    migrate_database(&settings).await;

    let key_generator = Arc::new(
        KeyGenerator::init(&settings.mnemonic_path).context("cannot initialize key generator")?,
    );

    let wallet_database = WalletDatabase::new(&settings)
        .await
        .context("cannot connect to wallet database")?;

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
    wallet.keep_sync_with_chain()?;

    let controller = Controller::start_ldk(
        settings.clone(),
        bitcoind_client,
        wallet.clone(),
        &key_generator.lightning_seed(),
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
}
