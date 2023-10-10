//! kld-mgr - a cli for deploying kld clusters

#![deny(missing_docs)]

use anyhow::{bail, Context, Result};
use clap::Parser;
use mgr::certs::{create_cockroachdb_certs, create_lightning_certs};
use mgr::secrets::{
    create_deploy_key, generate_disk_encryption_key, generate_mnemonic_and_macaroons,
};
use mgr::ssh::generate_key_pair;
use mgr::{config::ConfigFile, generate_nixos_flake, logging, Config, Host, NixosFlake};
use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use toml_example::traits::TomlExample;

#[derive(clap::Args, PartialEq, Debug, Clone)]
struct InstallArgs {
    /// Comma-separated lists of hosts to perform the install
    #[clap(long, default_value = "")]
    hosts: String,

    /// Kexec-tarball url to install from
    #[clap(
        long,
        default_value = "https://github.com/nix-community/nixos-images/releases/download/nixos-22.11/nixos-kexec-installer-x86_64-linux.tar.gz"
    )]
    kexec_url: String,

    /// Enables debug output in nixos-anywhere
    #[clap(long, action)]
    debug: bool,

    /// Do not reboot after installation
    #[clap(long, action)]
    no_reboot: bool,

    /// The mnemonic phrases and macaroons will automatically generate on remote server when kld first initialize.
    /// This benefits when you own your remote server and can physically backup mnemonic phrases and macaroons without any copy through the internet
    /// When you first initialize KLD, mnemonic phrases and macaroons will automatically be
    /// generated on your remote server. This is advantageous if you own your remote server,
    /// as you can physically back up your mnemonic phrases and macaroons without the need to
    /// transmit any copies over the internet.
    #[clap(long, default_value = "false")]
    generate_secret_on_remote: bool,
}

#[derive(clap::Args, PartialEq, Debug, Clone)]
struct GenerateConfigArgs {
    /// Directory where to copy the configuration to.
    directory: PathBuf,
}

#[derive(clap::Args, PartialEq, Debug, Clone)]
struct SystemInfoArgs {
    /// Comma-separated lists of hosts to perform the install
    #[clap(long, default_value = "")]
    hosts: String,
}

/// Subcommand to run
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(clap::Subcommand, PartialEq, Debug, Clone)]
enum Command {
    /// Generate NixOS configuration
    GenerateConfig(GenerateConfigArgs),
    /// Generate kld.toml example
    GenerateExample,
    /// Install kld cluster on given hosts. This will remove all data of the current system!
    Install(InstallArgs),
    /// Get system info from a host
    SystemInfo(SystemInfoArgs),
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// configuration file to load
    #[clap(long, default_value = "kld.toml", env = "KLD_CONFIG")]
    config: PathBuf,

    /// skip interactive dialogs by assuming the answer is yes
    #[clap(long, default_value = "false")]
    yes: bool,

    #[clap(subcommand)]
    action: Command,
}

fn ask_yes_no(prompt_text: &str) -> bool {
    println!("{prompt_text} ");
    let stdin = io::stdin();
    let mut line = String::new();
    if stdin.lock().read_line(&mut line).is_err() {
        return false;
    }
    let normalized = line.trim_end_matches('\n').to_string().to_ascii_lowercase();

    matches!(normalized.as_str(), "y" | "yes")
}

fn filter_hosts(host_spec: &str, hosts: &BTreeMap<String, Host>) -> Result<Vec<Host>> {
    if host_spec.is_empty() {
        return Ok(hosts.values().map(Clone::clone).collect::<Vec<_>>());
    }
    let mut filtered = vec![];
    for name in host_spec.split(',') {
        match hosts.get(name) {
            Some(v) => {
                filtered.push(v.clone());
            }
            None => {
                bail!("no host named '{}' found in configuration", name)
            }
        }
    }
    Ok(filtered)
}

fn install(
    args: &Args,
    install_args: &InstallArgs,
    config: &Config,
    flake: &NixosFlake,
) -> Result<()> {
    let hosts = filter_hosts(&install_args.hosts, &config.hosts)?;
    if !args.yes && !ask_yes_no(
            "Installing will remove any existing data from the configured hosts. Do you want to continue? (y/n)"
        ) {
        return Ok(());
    }
    mgr::install(
        &hosts,
        &install_args.kexec_url,
        flake,
        &config.global.secret_directory,
        &config.global.access_tokens,
        install_args.debug,
        install_args.no_reboot,
    )
}

fn generate_config(
    _args: &Args,
    config_args: &GenerateConfigArgs,
    _config: &Config,
    flake: &NixosFlake,
) -> Result<()> {
    mgr::generate_config(&config_args.directory, flake)
}

fn print_host_info(host: &Host) -> Result<()> {
    println!("[{}]", host.name);
    if let Ok(output) = std::process::Command::new("ssh")
        .args([
            host.deploy_ssh_target().as_str(),
            "--",
            "kld-ctl",
            "system-info",
        ])
        .output()
    {
        if output.status.success() {
            io::stdout().write_all(&output.stdout)?;
        } else {
            println!(
                "fetch system info of {} error: {}",
                host.name,
                std::str::from_utf8(&output.stderr).unwrap_or("fail to decode stderr")
            );
        }
    } else {
        println!("Fail to fetch system info from {}", host.name);
    }
    Ok(())
}

fn system_info(args: &SystemInfoArgs, config: &Config) -> Result<()> {
    println!("kld-mgr version: {}\n", env!("CARGO_PKG_VERSION"));
    let hosts = filter_hosts(&args.hosts, &config.hosts)?;
    for host in hosts {
        print_host_info(&host)?;
        println!("\n");
    }
    Ok(())
}

/// The kuutamo program entry point
pub fn main() -> Result<()> {
    logging::init().context("failed to initialize logging")?;
    let args = Args::parse();

    let res = match args.action {
        Command::GenerateExample => Ok(println!("{}", ConfigFile::toml_example())),
        Command::Install(ref install_args) => {
            let config =
                mgr::load_configuration(&args.config, !install_args.generate_secret_on_remote)
                    .with_context(|| {
                        format!(
                            "failed to parse configuration file: {}",
                            &args.config.display()
                        )
                    })?;
            create_deploy_key(&config.global.secret_directory)?;
            create_lightning_certs(
                &config.global.secret_directory.join("lightning"),
                &config.hosts,
            )
            .context("failed to create or update lightning certificates")?;
            create_cockroachdb_certs(
                &config.global.secret_directory.join("cockroachdb"),
                &config.hosts,
            )
            .context("failed to create or update cockroachdb certificates")?;

            // ssh server key for initrd sshd
            let sshd_dir = config.global.secret_directory.join("sshd");
            std::fs::create_dir_all(&sshd_dir)?;
            for (name, _) in config.hosts.iter() {
                let p = sshd_dir.join(name);
                if !p.exists() {
                    generate_key_pair(&p)?;
                }
            }

            let disk_encryption_key = &config.global.secret_directory.join("disk_encryption_key");
            if !disk_encryption_key.exists() {
                generate_disk_encryption_key(disk_encryption_key)?;
            }

            if !install_args.generate_secret_on_remote {
                generate_mnemonic_and_macaroons(&config.global.secret_directory)?;
            }
            let flake = generate_nixos_flake(&config).context("failed to generate flake")?;
            install(&args, install_args, &config, &flake)
        }
        _ => {
            let config = mgr::load_configuration(&args.config, false).with_context(|| {
                format!(
                    "failed to parse configuration file: {}",
                    &args.config.display()
                )
            })?;
            let flake = generate_nixos_flake(&config).context("failed to generate flake")?;
            match args.action {
                Command::GenerateConfig(ref config_args) => {
                    generate_config(&args, config_args, &config, &flake)
                }
                Command::SystemInfo(ref args) => system_info(args, &config),
                _ => unreachable!(),
            }
        }
    };
    res.with_context(|| format!("kuutamo failed doing: {:?}", args.action))
}
