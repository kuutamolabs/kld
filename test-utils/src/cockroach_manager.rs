use std::{fs, os::unix::prelude::PermissionsExt};

use crate::{
    connection,
    manager::{Manager, Starts},
    ports::get_available_port,
    TestSettingsBuilder,
};
use anyhow::Result;
use async_trait::async_trait;

pub struct CockroachManager {
    manager: Manager,
    port: u16,
    pub sql_port: u16,
    http_address: String,
    certs_dir: String,
}

impl CockroachManager {
    pub async fn start(&mut self) -> Result<()> {
        // Cockroach requires certs to be only read/writable by user in secure mode. Git does not track this.
        for file in fs::read_dir(&self.certs_dir)? {
            let file = file?;
            let mut perms = fs::metadata(file.path())?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(file.path(), perms)?;
        }
        let args = &[
            "start-single-node",
            &format!("--listen-addr=127.0.0.1:{}", self.port),
            &format!("--sql-addr=127.0.0.1:{}", self.sql_port),
            &format!("--http-addr={}", self.http_address),
            &format!("--store={}", self.manager.storage_dir),
            &format!("--certs-dir={}", self.certs_dir),
        ];
        self.manager.start("cockroach", args).await
    }

    pub fn test_cockroach(output_dir: &str, node_index: u16) -> CockroachManager {
        let port = get_available_port().expect("Cannot find free node port for cockroach");
        let http_port = get_available_port().expect("Cannot find free http port for cockroach");
        let sql_port = get_available_port().expect("Cannot find free sql port for cockroach");
        let http_address = format!("127.0.0.1:{http_port}");
        let certs_dir = format!("{}/certs/cockroach", env!("CARGO_MANIFEST_DIR"));

        let manager = Manager::new(
            Box::new(CockroachApi(sql_port)),
            output_dir,
            "cockroach",
            node_index,
        );
        CockroachManager {
            manager,
            port,
            sql_port,
            http_address,
            certs_dir,
        }
    }

    pub fn kill(&mut self) {
        self.manager.kill()
    }
}

pub struct CockroachApi(u16);

#[async_trait]
impl Starts for CockroachApi {
    async fn has_started(&self, _manager: &Manager) -> bool {
        let settings = TestSettingsBuilder::new()
            .with_database_port(self.0)
            .build();
        connection(&settings).await.is_ok()
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
