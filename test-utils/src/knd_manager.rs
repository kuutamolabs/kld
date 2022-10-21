use crate::bitcoin_manager::BitcoinManager;
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
}

impl KndManager {
    pub fn start(&mut self) {
        if self.process.is_none() {
            let log_file = File::create(format!("{}/test.log", self.storage_dir)).unwrap();
            let fd = log_file.as_raw_fd();
            let out = unsafe { Stdio::from_raw_fd(fd) };
            let child = Command::new(&self.bin_path).stdout(out).spawn().unwrap();
            self.process = Some(child)
        }
    }

    pub fn kill(&mut self) {
        if let Some(mut process) = self.process.take() {
            process.kill().unwrap_or_default();
            process.wait().unwrap();
            self.process = None
        }
    }

    pub fn test_knd(
        bin_path: &str,
        test_name: &str,
        node_index: u16,
        bitcoin: &BitcoinManager,
    ) -> KndManager {
        let test_number = std::fs::read_dir("tests")
            .unwrap()
            .position(|f| f.unwrap().file_name().to_str().unwrap() == format!("{}.rs", test_name))
            .unwrap() as u16;

        let port = 20000u16 + (test_number * 1000u16) + node_index * 10;
        let current_dir = std::env::current_dir().unwrap().display().to_string();
        let storage_dir = format!("{}/output/{}/knd_{}", current_dir, test_name, node_index);

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
        set_var("KND_PEER_PORT", &port.to_string());
        set_var("BITCOIN_NETWORK", &bitcoin.network);
        set_var("BITCOIN_COOKIE_PATH", &bitcoin.cookie_path());
        set_var("BITCOIN_RPC_HOST", "127.0.0.1");
        set_var("BITCOIN_RPC_PORT", &bitcoin.rpc_port.to_string());
        KndManager {
            process: None,
            bin_path: bin_path.to_string(),
            storage_dir,
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
    ($bitcoin:expr) => {
        test_utils::knd_manager::KndManager::test_knd(
            env!("CARGO_BIN_EXE_lightning-knd"),
            env!("CARGO_CRATE_NAME"),
            0,
            $bitcoin,
        )
    };
    ($n:literal, $bitcoin:expr) => {
        test_utils::knd_manager::KndManager::test_knd(
            env!("CARGO_BIN_EXE_lightning-knd"),
            env!("CARGO_CRATE_NAME"),
            $n,
            $bitcoin,
        )
    };
}
