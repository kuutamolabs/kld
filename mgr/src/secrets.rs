use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::{fs, path::Path};

use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine};
use bip39::Mnemonic;
use bitcoin::hashes::{sha256, Hash, HashEngine};
use macaroon::{Macaroon, MacaroonKey};
use rand::thread_rng;
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

pub fn generate_mnemonic_and_macaroons(secret_directory: &Path) -> Result<()> {
    let mnemonic_path = secret_directory.join("mnemonic");
    let mnemonic = if !mnemonic_path.exists() {
        let mut rng = thread_rng();
        let mnemonic = Mnemonic::generate_in_with(&mut rng, bip39::Language::English, 24)?;
        fs::write(&mnemonic_path, mnemonic.to_string())
            .with_context(|| format!("Cannot write to {}", mnemonic_path.display()))?;

        println!("Generated a new mnemonic: {}", mnemonic_path.display());
        mnemonic
    } else if let Ok(words) = fs::read_to_string(mnemonic_path) {
        Mnemonic::parse(words)?
    } else {
        panic!("mnemonic is incorrect")
    };

    let mut engine = sha256::HashEngine::default();
    engine.input(&mnemonic.to_seed(""));
    engine.input("macaroon/0".as_bytes());
    let hash = sha256::Hash::from_engine(engine);
    let seed = hash.into_inner();

    let key = MacaroonKey::generate(&seed);
    let mut admin_macaroon = Macaroon::create(None, &key, "admin".into())?;
    admin_macaroon.add_first_party_caveat("roles = admin|readonly".into());

    let mut readonly_macaroon = Macaroon::create(None, &key, "readonly".into())?;
    readonly_macaroon.add_first_party_caveat("roles = readonly".into());

    let mut buf = vec![];
    let base64 = admin_macaroon.serialize(macaroon::Format::V2)?;
    general_purpose::URL_SAFE.decode_vec(base64, &mut buf)?;

    fs::write(secret_directory.join("access.macaroon"), &buf)?;
    fs::write(
        secret_directory.join("admin.macaroon"),
        admin_macaroon.serialize(macaroon::Format::V2)?,
    )?;
    fs::write(
        secret_directory.join("readonly.macaroon"),
        readonly_macaroon.serialize(macaroon::Format::V2)?,
    )?;

    Ok(())
}
