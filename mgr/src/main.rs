//! kld-mgr - a cli for deploying kld clusters

#![deny(missing_docs)]

use anyhow::{bail, Context, Result};
use clap::Parser;
use mgr::lightning_certs::{create_or_update_lightning_certs, RenewPolicy};
use mgr::{generate_nixos_flake, logging, Config, Host, NixosFlake};
use std::collections::HashMap;
use std::io::{self, BufRead};
use std::path::PathBuf;

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
}

#[derive(clap::Args, PartialEq, Debug, Clone)]
struct GenerateConfigArgs {
    /// Directory where to copy the configuration to.
    directory: PathBuf,
}

#[derive(clap::Args, PartialEq, Debug, Clone)]
struct DryUpdateArgs {
    /// Comma-separated lists of hosts to perform the dry-update
    #[clap(long, default_value = "")]
    hosts: String,
}

#[derive(clap::Args, PartialEq, Debug, Clone)]
struct UpdateArgs {
    /// Comma-separated lists of hosts to perform the update
    #[clap(long, default_value = "")]
    hosts: String,
}

/// Subcommand to run
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(clap::Subcommand, PartialEq, Debug, Clone)]
enum Command {
    /// Generate NixOS configuration
    GenerateConfig(GenerateConfigArgs),
    /// Install Validator on a given machine. This will remove all data of the current system!
    Install(InstallArgs),
    /// Upload update to host and show which actions would be performed on an update
    DryUpdate(DryUpdateArgs),
    /// Update validator
    Update(UpdateArgs),
    Rollback(RollbackArgs),
}

#[derive(clap::Args, PartialEq, Debug, Clone)]
struct RollbackArgs {
    /// Comma-separated lists of hosts to perform the rollback
    #[clap(long, default_value = "")]
    hosts: String,
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

fn filter_hosts(host_spec: &str, hosts: &HashMap<String, Host>) -> Result<Vec<Host>> {
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

fn dry_update(
    _args: &Args,
    dry_update_args: &DryUpdateArgs,
    config: &Config,
    flake: &NixosFlake,
) -> Result<()> {
    let hosts = filter_hosts(&dry_update_args.hosts, &config.hosts)?;
    mgr::dry_update(&hosts, flake)
}

fn update(
    _args: &Args,
    update_args: &UpdateArgs,
    config: &Config,
    flake: &NixosFlake,
) -> Result<()> {
    let hosts = filter_hosts(&update_args.hosts, &config.hosts)?;
    mgr::update(&hosts, flake)
}

fn rollback(
    _args: &Args,
    rollback_args: &RollbackArgs,
    config: &Config,
    flake: &NixosFlake,
) -> Result<()> {
    let hosts = filter_hosts(&rollback_args.hosts, &config.hosts)?;
    mgr::rollback(&hosts, flake)
}

/// The kuutamo program entry point
pub fn main() -> Result<()> {
    logging::init().context("failed to initialize logging")?;
    let args = Args::parse();
    let config = mgr::load_configuration(&args.config).with_context(|| {
        format!(
            "failed to parse configuration file: {}",
            &args.config.display()
        )
    })?;
    let flake = generate_nixos_flake(&config).context("failed to generate flake")?;

    if let Err(e) = match args.action {
        Command::GenerateConfig(ref config_args) => {
            generate_config(&args, config_args, &config, &flake)
        }
        Command::Install(ref install_args) => install(&args, install_args, &config, &flake),
        Command::DryUpdate(ref dry_update_args) => {
            dry_update(&args, dry_update_args, &config, &flake)
        }
        Command::Update(ref update_args) => update(&args, update_args, &config, &flake),
        Command::Rollback(ref rollback_args) => rollback(&args, rollback_args, &config, &flake),
    } {
        bail!("kuutamo failed doing {:?}: {e}", args.action);
    }
    Ok(())
}
