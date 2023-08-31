use anyhow::{Context, Result};
use async_trait::async_trait;
use std::{
    fs::{self, File},
    io::Read,
    marker::PhantomData,
    os::unix::prelude::{AsRawFd, FromRawFd},
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use tempfile::TempDir;

pub struct Manager<'a> {
    pub process: Option<Child>,
    phantom: PhantomData<&'a TempDir>,
    pub storage_dir: PathBuf,
    instance_name: String,
}

impl<'a> Manager<'a> {
    pub fn new(output_dir: &'a TempDir, name: &str, instance: &str) -> Result<Self> {
        let instance_name = format!("{name}_{instance}");
        let storage_dir = output_dir.path().join(&instance_name);
        fs::create_dir(&storage_dir)?;

        Ok(Manager {
            process: None,
            phantom: PhantomData,
            storage_dir,
            instance_name,
        })
    }

    pub async fn start(&mut self, command: &str, args: &[&str], check: impl Check) -> Result<()> {
        if self.process.is_none() {
            let path = format!("{}/test.log", self.storage_dir.as_path().display());
            let log_file = File::create(&path).unwrap();
            let fd = log_file.as_raw_fd();
            let out = unsafe { Stdio::from_raw_fd(fd) };
            let err = unsafe { Stdio::from_raw_fd(fd) };
            println!("Starting: {}", self.instance_name);
            let child = Command::new(command)
                .stdout(out)
                .stderr(err)
                .args(args)
                .spawn()
                .with_context(|| format!("failed to start {command}"))?;

            self.process = Some(child);

            let i = Instant::now();
            let started = loop {
                if check.check().await {
                    break true;
                };
                if i.elapsed() >= Duration::from_secs(60) {
                    break false;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            };
            if !started {
                let mut file = File::open(&path).unwrap();
                let mut buf = String::new();
                file.read_to_string(&mut buf).unwrap();
                println!("Timed out waiting to start: {}", self.instance_name);
                println!("Begin log file: {path}");
                println!("{buf}");
                println!("End of log file.");
                panic!("Failed to start {}", self.instance_name);
            } else {
                println!("Successfully started: {}", self.instance_name);
            }
        }
        Ok(())
    }

    pub fn kill(&mut self) {
        if let Some(mut process) = self.process.take() {
            println!("Stopping: {}", self.instance_name);
            if let Ok(Some(_)) = process.try_wait() {
                return;
            }
            let _ = Command::new("kill").arg(process.id().to_string()).output();
            let mut count = 0;
            while count < 30 {
                if let Ok(Some(_)) = process.try_wait() {
                    println!("{} stopped after {count} secs", self.instance_name);
                    return;
                }
                std::thread::sleep(Duration::from_secs(1));
                count += 1;
            }
            println!("Killing: {}", self.instance_name);
            process.kill().unwrap_or_default();
            process.wait().unwrap();
            self.process = None
        }
    }
}

#[async_trait]
pub trait Check {
    async fn check(&self) -> bool;
}

impl Drop for Manager<'_> {
    fn drop(&mut self) {
        self.kill()
    }
}
