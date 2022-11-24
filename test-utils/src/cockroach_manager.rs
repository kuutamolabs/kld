use crate::{poll, unique_number};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub struct CockroachManager {
    process: Option<Child>,
    storage_dir: String,
    pub port: u16,
    http_address: String,
}

impl CockroachManager {
    pub async fn start(&mut self) {
        if self.process.is_none() {
            let child = Command::new("cockroach")
                .arg("start-single-node")
                .arg(format!("--insecure"))
                .arg(format!("--listen-addr=127.0.0.1:{}", self.port))
                .arg(format!("--http-addr={}", self.http_address))
                .arg(format!("--store={}", self.storage_dir))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();

            self.process = Some(child);

            poll!(5, self.has_started().await);
        }
    }

    pub fn kill(&mut self) {
        if let Some(mut process) = self.process.take() {
            process.kill().unwrap_or_default();
            process.wait().unwrap();
            self.process = None
        }
    }

    pub async fn has_started(&self) -> bool {
        reqwest::get(format!("http://{}", self.http_address.clone()))
            .await
            .is_ok()
    }

    pub fn test_cockroach(output_dir: &str, node_index: u16) -> CockroachManager {
        let name = std::thread::current().name().unwrap().to_string();
        let n = unique_number();

        let port = 50000u16 + (n * 1000u16) + (node_index * 10);
        let storage_dir = format!("{}/{}_cockroach_{}", output_dir, name, node_index);
        let http_address = format!("127.0.0.1:{}", port + 1);

        std::fs::remove_dir_all(&storage_dir).unwrap_or_default();
        std::fs::create_dir_all(&storage_dir).unwrap();

        CockroachManager {
            process: None,
            storage_dir,
            port,
            http_address,
        }
    }
}

impl Drop for CockroachManager {
    fn drop(&mut self) {
        self.kill()
    }
}

#[macro_export]
macro_rules! cockroach {
    () => {
        test_utils::cockroach_manager::CockroachManager::test_cockroach(
            env!("CARGO_TARGET_TMPDIR"),
            0,
        )
    };
    ($n:literal) => {
        test_utils::cockroach_manager::CockroachManager::test_cockroach(
            env!("CARGO_TARGET_TMPDIR"),
            $n,
        )
    };
}
