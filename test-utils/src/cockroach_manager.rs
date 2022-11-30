use crate::manager::Manager;

pub struct CockroachManager {
    manager: Manager,
    pub port: u16,
    http_address: String,
}

impl CockroachManager {
    pub async fn start(&mut self) {
        let args = &[
            "start-single-node",
            "--insecure",
            &format!("--listen-addr=127.0.0.1:{}", self.port),
            &format!("--http-addr={}", self.http_address),
            &format!("--store={}", self.manager.storage_dir),
        ];
        self.manager.start("cockroach", args).await
    }

    pub fn test_cockroach(output_dir: &str) -> CockroachManager {
        let port = 50000u16;
        let http_address = format!("127.0.0.1:{}", port + 1);

        let manager = Manager::new(
            output_dir,
            "cockroach",
            0,
            format!("http://{}", http_address.clone()),
        );
        CockroachManager {
            manager,
            port,
            http_address,
        }
    }

    pub fn kill(&mut self) {
        self.manager.kill()
    }
}

#[macro_export]
macro_rules! cockroach {
    () => {
        test_utils::cockroach_manager::CockroachManager::test_cockroach(env!("CARGO_TARGET_TMPDIR"))
    };
}
