use std::fs::File;
use std::marker::PhantomData;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result};
use kld::settings::Settings;
use tempfile::TempDir;
use tokio::time::{sleep_until, Instant};

use crate::{ports::get_available_port, BitcoinManager};

pub struct ElectrsManager<'a, 'b> {
    process: Child,
    phantom: PhantomData<&'a TempDir>,
    bitcoind: PhantomData<&'b BitcoinManager<'b>>,
    pub monitoring_addr: String,
}

impl<'a, 'b> ElectrsManager<'a, 'b> {
    pub async fn new(
        output_dir: &'a TempDir,
        bitcoin_manager: &'b BitcoinManager<'b>,
        settings: &mut Settings,
    ) -> Result<ElectrsManager<'a, 'b>> {
        let monitoring_port = get_available_port()?;
        let rpc_port = get_available_port()?;
        let storage_dir = output_dir
            .path()
            .join(format!("electrs_{}", settings.node_id));
        std::fs::create_dir(&storage_dir)?;

        let mut args = vec![
            "--skip-default-conf-files".into(),
            format!("--network={}", &settings.bitcoin_network),
            format!("--db-dir={}", &storage_dir.as_path().display()),
            format!("--cookie-file={}", settings.bitcoin_cookie_path),
            format!("--electrum-rpc-addr=127.0.0.1:{rpc_port}"),
            format!("--daemon-rpc-addr=127.0.0.1:{}", settings.bitcoind_rpc_port),
            format!("--daemon-p2p-addr=127.0.0.1:{}", bitcoin_manager.p2p_port),
            format!("--monitoring-addr=127.0.0.1:{monitoring_port}"),
        ];
        if std::env::var("KEEP_TEST_ARTIFACTS_IN").is_ok() {
            args.push("--log-filters=DEBUG".into());
        }
        let process = if std::env::var("KEEP_TEST_ARTIFACTS_IN").is_ok() {
            Command::new("electrs")
                .args(args)
                .stdout(Stdio::null())
                .stderr(File::create(storage_dir.join("electrs.log"))?)
                .spawn()
                .with_context(|| "failed to start electrs")?
        } else {
            Command::new("electrs")
                .args(args)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .with_context(|| "failed to start electrs")?
        };

        let electrs = ElectrsManager {
            process,
            monitoring_addr: format!("127.0.0.1:{monitoring_port}"),
            phantom: PhantomData,
            bitcoind: PhantomData,
        };
        settings.electrs_url = format!("127.0.0.1:{rpc_port}");

        let mut count = 0;
        while let Err(e) = reqwest::get(&format!("http://{}", electrs.monitoring_addr))
            .await
            .with_context(|| "could not monitor on electrs")
        {
            if count > 3 {
                return Err(e);
            } else {
                sleep_until(Instant::now() + Duration::from_secs(1)).await;
                count += 1;
            }
        }
        Ok(electrs)
    }
}

impl Drop for ElectrsManager<'_, '_> {
    fn drop(&mut self) {
        // Report unexpected close, try kill the electrs process and wait the log
        match self.process.try_wait() {
            Ok(Some(status)) => eprintln!("electrs exited unexpected, status code: {status}"),
            Ok(None) => {
                let _ = Command::new("kill")
                    .arg(self.process.id().to_string())
                    .output();
                let mut count = 0;
                while count < 5 {
                    if let Ok(Some(_)) = self.process.try_wait() {
                        return;
                    }
                    std::thread::sleep(Duration::from_secs(1 + count * 3));
                    count += 1;
                }
                self.process.kill().expect("electrs couldn't be killed");
            }
            Err(_) => panic!("error attempting electrs status"),
        }
    }
}
