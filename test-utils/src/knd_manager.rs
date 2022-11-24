use crate::bitcoin_manager::BitcoinManager;
use crate::cockroach_manager::CockroachManager;
use crate::{poll, unique_number};
use std::env::set_var;
use std::fs::File;
use std::os::unix::prelude::{AsRawFd, FromRawFd};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

pub struct KndManager {
    process: Option<Child>,
    bin_path: String,
    storage_dir: String,
    exporter_address: String,
}

impl KndManager {
    pub async fn start(&mut self) {
        if self.process.is_none() {
            let log_file = File::create(format!("{}/test.log", self.storage_dir)).unwrap();
            let fd = log_file.as_raw_fd();
            let out = unsafe { Stdio::from_raw_fd(fd) };
            let err = unsafe { Stdio::from_raw_fd(fd) };
            let child = Command::new(&self.bin_path)
                .stdout(out)
                .stderr(err)
                .spawn()
                .unwrap();
            self.process = Some(child);

            // Wait for full startup before returning.
            poll!(5, self.call_exporter("health").await.is_ok());
        }
    }

    pub fn kill(&mut self) {
        if let Some(mut process) = self.process.take() {
            process.kill().unwrap_or_default();
            process.wait().unwrap();
            self.process = None
        }
    }

    pub fn pid(&self) -> Option<u32> {
        self.process.as_ref().map(|p| p.id())
    }

    pub async fn call_exporter(&self, method: &str) -> Result<String, reqwest::Error> {
        reqwest::get(format!("http://{}/{}", self.exporter_address, method))
            .await?
            .text()
            .await
    }

    pub fn test_knd(
        output_dir: &str,
        bin_path: &str,
        node_index: u16,
        bitcoin: &BitcoinManager,
        cockroach: &CockroachManager,
    ) -> KndManager {
        let test_name = std::thread::current().name().unwrap().to_string();
        let n = unique_number();

        let peer_port = 40000u16 + (n * 1000u16) + node_index * 10;
        let storage_dir = format!("{}/{}/knd_{}", output_dir, test_name, node_index);
        let exporter_address = format!("127.0.0.1:{}", peer_port + 1);

        std::fs::remove_dir_all(&storage_dir).unwrap_or_default();
        std::fs::create_dir_all(&storage_dir).unwrap();

        if node_index <= 1 {
            let _ = Command::new("killall")
                .arg("lightning_knd")
                .stdout(Stdio::null())
                .output();
            sleep(Duration::from_secs(1))
        }

        set_var("KND_STORAGE_DIR", &storage_dir);
        set_var("KND_PEER_PORT", &peer_port.to_string());
        set_var("KND_EXPORTER_ADDRESS", &exporter_address);
        set_var("KND_BITCOIN_NETWORK", &bitcoin.network);
        set_var("KND_BITCOIN_COOKIE_PATH", &bitcoin.cookie_path());
        set_var("KND_BITCOIN_RPC_HOST", "127.0.0.1");
        set_var("KND_BITCOIN_RPC_PORT", &bitcoin.rpc_port.to_string());
        set_var("KND_DATABASE_PORT", &cockroach.port.to_string());

        KndManager {
            process: None,
            bin_path: bin_path.to_string(),
            storage_dir,
            exporter_address,
        }
    }
}

impl Drop for KndManager {
    fn drop(&mut self) {
        self.kill()
    }
}

#[macro_export]
macro_rules! knd {
    ($bitcoin:expr, $cockroach:expr) => {
        test_utils::knd_manager::KndManager::test_knd(
            env!("CARGO_TARGET_TMPDIR"),
            env!("CARGO_BIN_EXE_lightning-knd"),
            0,
            $bitcoin,
            $cockroach,
        )
    };
    ($n:literal, $bitcoin:expr, $cockroach:expr) => {
        test_utils::knd_manager::KndManager::test_knd(
            env!("CARGO_TARGET_TMPDIR"),
            env!("CARGO_BIN_EXE_lightning-knd"),
            $n,
            $bitcoin,
            $cockroach,
        )
    };
}
