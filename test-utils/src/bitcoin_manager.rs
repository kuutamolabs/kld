use std::marker::PhantomData;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use bitcoin::Address;
use kld::bitcoind::BitcoindClient;
use kld::settings::Network;
use kld::settings::Settings;
use std::process::{Child, Command, Stdio};
use tempfile::TempDir;
use tokio::time::{sleep_until, Instant};

use crate::ports::get_available_port;

pub struct BitcoinManager<'a> {
    process: Child,
    phantom: PhantomData<&'a TempDir>,
    pub p2p_port: u16,
    pub client: BitcoindClient,
}

impl<'a> BitcoinManager<'a> {
    pub async fn new(
        output_dir: &'a TempDir,
        settings: &mut Settings,
    ) -> Result<BitcoinManager<'a>> {
        let p2p_port = get_available_port()?;
        let rpc_port = get_available_port()?;
        let storage_dir = output_dir
            .path()
            .join(&format!("bitcoind_{}", settings.node_id));
        std::fs::create_dir(&storage_dir)?;

        settings.bitcoind_rpc_port = rpc_port;
        settings.bitcoin_cookie_path = if settings.bitcoin_network == Network::Main {
            storage_dir.join(".cookie")
        } else {
            storage_dir
                .join(settings.bitcoin_network.to_string())
                .join(".cookie")
        }
        .into_os_string()
        .into_string()
        .expect("should not use non UTF-8 char in path");

        let mut args = vec![
            "-server".into(),
            "-noconnect".into(),
            "-rpcthreads=1".into(),
            "-listen".into(),
            format!("-chain={}", settings.bitcoin_network),
            format!("-datadir={}", storage_dir.display()),
            format!("-port={}", p2p_port),
            format!("-rpcport={}", rpc_port),
        ];
        if std::env::var("KEEP_TEST_ARTIFACTS_IN").is_ok() {
            args.push(format!(
                "-debuglogfile={}",
                storage_dir.join("bitcoind.log").display()
            ));
        }

        let mut process = Command::new("bitcoind")
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| "failed to start bitcoind")?;

        // XXX
        // Request `call` and `new` of BitcoindClient should be separate functions
        // It is not `BitcoindConnection`, so it doesn't make sense to have a hidden request in new.
        // Also the manager is hard to init the client with `bitcoind` at same time.
        let mut count = 0;
        while let Err(e) = BitcoindClient::new(settings)
            .await
            .with_context(|| "could not connect to bitcoind")
        {
            if count > 3 {
                let _ = process.kill();
                return Err(e);
            } else {
                sleep_until(Instant::now() + Duration::from_secs(1 + count * 3)).await;
                count += 1;
            }
        }

        let bitcoind = match BitcoindClient::new(settings).await {
            Ok(client) => BitcoinManager {
                process,
                phantom: PhantomData,
                p2p_port,
                client,
            },
            Err(_) => {
                let _ = process.kill();
                bail!("fail to make bitcoind client")
            }
        };

        Ok(bitcoind)
    }

    pub async fn generate_blocks(
        &self,
        n_blocks: u64,
        address: &Address,
        delay: bool,
    ) -> Result<()> {
        for _ in 0..n_blocks {
            // Sometimes a delay is needed to make the test more realistic which is expected by LDK.
            if delay {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            self.client.generate_to_address(1, address).await?;
        }
        self.client.wait_for_blockchain_synchronisation().await;
        Ok(())
    }
}

impl Drop for BitcoinManager<'_> {
    fn drop(&mut self) {
        // Report unexpected bitcoind close, try kill the bitcoind process and wait the log
        match self.process.try_wait() {
            Ok(Some(status)) => eprintln!("bitcoind exited unexpected, status code: {status}"),
            Ok(None) => {
                let _ = Command::new("kill")
                    .arg(self.process.id().to_string())
                    .output();
                std::thread::sleep(Duration::from_secs(1));
                if let Ok(Some(_)) = self.process.try_wait() {
                } else {
                    self.process.kill().expect("bitcoind couldn't be killed");
                }
            }
            Err(_) => panic!("error attempting bitcoind status"),
        }
    }
}
