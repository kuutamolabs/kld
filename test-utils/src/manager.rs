use anyhow::{Context, Result};
use std::{
    fs::{self, File},
    io::Read,
    os::unix::prelude::{AsRawFd, FromRawFd},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use async_trait::async_trait;

pub struct Manager {
    pub process: Option<Child>,
    pub storage_dir: String,
    instance_name: String,
    starts: Box<dyn Starts + Send + Sync>,
}

impl Manager {
    pub fn new(
        starts: Box<dyn Starts + Send + Sync>,
        output_dir: &str,
        name: &str,
        node_index: u16,
    ) -> Self {
        let instance_name = format!("{}_{}", name, node_index);
        let storage_dir = format!("{}/{}", output_dir, instance_name);
        // Getting occasional bad file descriptors with fs::remove_dir_all so try this instead.
        let _ = Command::new("rm").args(["-rf", &storage_dir]).output();
        fs::create_dir_all(&storage_dir).unwrap();

        Manager {
            process: None,
            storage_dir,
            instance_name,
            starts,
        }
    }

    pub async fn start(&mut self, command: &str, args: &[&str]) -> Result<()> {
        if self.process.is_none() {
            let path = format!("{}/test.log", self.storage_dir);
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
                .with_context(|| format!("failed to start {}", command))?;

            self.process = Some(child);

            let i = Instant::now();
            let started = loop {
                if self.starts.has_started(self).await {
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
                println!("Begin log file: {}", path);
                println!("{}", buf);
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
pub trait Starts {
    async fn has_started(&self, manager: &Manager) -> bool;
}

impl Drop for Manager {
    fn drop(&mut self) {
        self.kill()
    }
}
