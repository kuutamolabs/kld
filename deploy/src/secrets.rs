use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::{fs, path::Path};

use anyhow::{Context, Result};
use tempfile::{Builder, TempDir};

use crate::command::status_to_pretty_err;

pub struct Secrets {
    tmp_dir: TempDir,
}

impl Secrets {
    pub fn new<'a, I>(secrets: I) -> Result<Self>
    where
        I: Iterator<Item = &'a (PathBuf, String)>,
    {
        let tmp_dir = Builder::new()
            .prefix("kuutamo-secrets.")
            .tempdir()
            .context("cannot create temporary directory")?;

        let mut options = OpenOptions::new();
        options.mode(0o600);
        options.write(true);
        options.create(true);
        for (to, content) in secrets {
            let secret_path = tmp_dir.path().join(to.strip_prefix("/").unwrap_or(to));
            let dir = secret_path.parent().with_context(|| {
                format!("Cannot get parent of directory: {}", secret_path.display())
            })?;
            fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;

            let mut file = options.open(&secret_path).with_context(|| {
                format!("Cannot open secret {} for writing.", secret_path.display())
            })?;
            file.write_all(content.as_bytes()).with_context(|| {
                format!(
                    "cannot write secret to temporary location at {}",
                    secret_path.display()
                )
            })?;
        }
        Ok(Self { tmp_dir })
    }
    /// Path to the nixos flake
    pub fn path(&self) -> &Path {
        self.tmp_dir.path()
    }

    // rsync -vrlF -e "ssh -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no" "$extra_files" "${ssh_connection}:/mnt/"
    pub fn upload(&self, ssh_target: &str) -> Result<()> {
        // Do proper logging here?
        println!("Upload secrets");
        let path = self
            .path()
            .to_str()
            .context("Cannot convert secrets directory to string")?;
        let rsync_target = format!("{ssh_target}:/");
        let rsync_path = format!("{path}/");
        let args = vec!["-vrlF", "-e", "ssh", &rsync_path, &rsync_target];
        let status = Command::new("rsync").args(&args).status();
        status_to_pretty_err(status, "rsync", &args)?;
        Ok(())
    }
}
