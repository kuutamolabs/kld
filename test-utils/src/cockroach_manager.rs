use crate::{
    manager::{Manager, Starts},
    ports::get_available_port,
};
use async_trait::async_trait;

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
        let port = get_available_port().expect("Cannot find free port for cockroach");
        let http_port = get_available_port().expect("Cannot find free http port for cockroach");
        let http_address = format!("127.0.0.1:{}", http_port);

        let manager = Manager::new(
            Box::new(CockroachApi(http_address.clone())),
            output_dir,
            "cockroach",
            0,
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

pub struct CockroachApi(String);

#[async_trait]
impl Starts for CockroachApi {
    async fn has_started(&self) -> bool {
        reqwest::get(format!("http://{}", self.0)).await.is_ok()
    }
}

#[macro_export]
macro_rules! cockroach {
    () => {
        test_utils::cockroach_manager::CockroachManager::test_cockroach(env!("CARGO_TARGET_TMPDIR"))
    };
}
