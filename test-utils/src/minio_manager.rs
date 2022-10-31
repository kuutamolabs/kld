use crate::poll;
use std::env::set_var;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub struct MinioManager {
    process: Option<Child>,
    storage_dir: String,
    address: String,
}

impl MinioManager {
    pub async fn start(&mut self) {
        if self.process.is_none() {
            let child = Command::new("minio")
                .arg("server")
                .arg(format!("--address={}", self.address))
                .arg(format!("--certs-dir={}/certs", self.storage_dir))
                .arg(self.storage_dir.clone())
                .stdout(Stdio::null())
                .spawn()
                .unwrap();

            self.process = Some(child);

            poll!(5, self.has_started_api().await);
        }
    }

    pub fn kill(&mut self) {
        if let Some(mut process) = self.process.take() {
            process.kill().unwrap_or_default();
            process.wait().unwrap();
            self.process = None
        }
    }

    pub async fn has_started_api(&self) -> bool {
        reqwest::get(format!("http://{}", self.address.clone()))
            .await
            .is_ok()
    }

    pub fn test_minio(output_dir: &str, test_name: &str) -> MinioManager {
        let test_number = std::fs::read_dir("tests")
            .unwrap()
            .position(|f| f.unwrap().file_name().to_str().unwrap() == format!("{}.rs", test_name))
            .unwrap() as u16;

        let port = 50000u16 + (test_number * 1000u16);
        let storage_dir = format!("{}/{}/minio", output_dir, test_name);
        let address = format!("127.0.0.1:{}", port);
        set_var("KND_S3_ADDRESS", &address);

        std::fs::remove_dir_all(&storage_dir).unwrap_or_default();

        let certs_dir = format!("{}/certs", &storage_dir);
        std::fs::create_dir_all(&certs_dir).unwrap();

        Command::new("certgen")
            .current_dir(certs_dir)
            .arg("-host=127.0.0.1")
            .stdout(Stdio::null())
            .output()
            .unwrap();

        MinioManager {
            process: None,
            storage_dir,
            address,
        }
    }
}

impl Drop for MinioManager {
    fn drop(&mut self) {
        self.kill()
    }
}

#[macro_export]
macro_rules! minio {
    () => {
        test_utils::minio_manager::MinioManager::test_minio(
            env!("CARGO_TARGET_TMPDIR"),
            env!("CARGO_CRATE_NAME"),
        )
    };
}
