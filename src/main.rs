use anyhow::{Context, Result};
use database::ldk_database::LdkDatabase;
use database::migrate_database;
use database::wallet_database::WalletDatabase;
use futures::FutureExt;
use kld::api::{bind_api_server, MacaroonAuth};
use kld::bitcoind::BitcoindClient;
use kld::key_generator::KeyGenerator;
use kld::ldk::Controller;
use kld::prometheus::start_prometheus_exporter;
use kld::quit_signal;
use kld::wallet::Wallet;
use log::{error, info};
use settings::Settings;
use std::sync::Arc;
use std::time::Duration;

pub fn main() -> Result<()> {
    let settings = Arc::new(Settings::load());
    logger::KldLogger::init(
        &settings.node_id,
        settings.log_level.parse().context("Invalid log level")?,
    );

    info!("Starting Kuutamo Lightning Daemon");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .build()?;

    if let Err(e) = runtime.block_on(run_kld(settings)) {
        error!("Fatal error running KND: {}", e);
    }

    info!("Shutting down");
    runtime.shutdown_timeout(Duration::from_secs(30));
    info!("Stopped all threads. Process finished.");
    Ok(())
}

async fn run_kld(settings: Arc<Settings>) -> Result<()> {
    let quit_signal = quit_signal().shared();

    migrate_database(&settings).await?;

    let key_generator = Arc::new(
        KeyGenerator::init(&settings.mnemonic_path).context("cannot initialize key generator")?,
    );

    let database = Arc::new(
        LdkDatabase::new(&settings)
            .await
            .context("cannot connect to ldk database")?,
    );
    let wallet_database = WalletDatabase::new(&settings)
        .await
        .context("cannot connect to wallet database")?;

    let bitcoind_client = Arc::new(BitcoindClient::new(&settings).await?);
    bitcoind_client
        .wait_for_blockchain_synchronisation()
        .await?;
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

    let controller = Controller::start_ldk(
        settings.clone(),
        database,
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
