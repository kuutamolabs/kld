//! kld-mgr - a cli for deploying kld clusters

#![deny(missing_docs)]

use anyhow::{bail, Context, Result};
use clap::Parser;
use mgr::certs::{
    create_or_update_cockroachdb_certs, create_or_update_lightning_certs, CertRenewPolicy,
};
use mgr::{generate_nixos_flake, logging, Config, Host, NixosFlake};
use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
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

#[derive(clap::Args, PartialEq, Debug, Clone)]
struct SshArgs {
    /// Host to ssh into
    #[clap(long, default_value = "")]
    hosts: String,

    /// Additional arguments to pass to ssh
    command: Option<Vec<String>>,
}

#[derive(clap::Args, PartialEq, Debug, Clone)]
struct RebootArgs {
    /// Comma-separated lists of hosts to perform the reboot
    #[clap(long, default_value = "")]
    hosts: String,
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
    /// Install kld cluster on given hosts. This will remove all data of the current system!
    Install(InstallArgs),
    /// Upload update to host and show which actions would be performed on an update
    DryUpdate(DryUpdateArgs),
    /// Update hosts
    Update(UpdateArgs),
    /// Rollback hosts to previous generation
    Rollback(RollbackArgs),
    /// SSH into a host
    Ssh(SshArgs),
    /// Reboot hosts
    Reboot(RebootArgs),
    /// Get system info from a host
    SystemInfo(SystemInfoArgs),
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
    mgr::dry_update(&hosts, flake, &config.global.secret_directory)
}

fn update(
    _args: &Args,
    update_args: &UpdateArgs,
    config: &Config,
    flake: &NixosFlake,
) -> Result<()> {
    let hosts = filter_hosts(&update_args.hosts, &config.hosts)?;
    mgr::update(&hosts, flake, &config.global.secret_directory)
}

fn rollback(
    _args: &Args,
    rollback_args: &RollbackArgs,
    config: &Config,
    flake: &NixosFlake,
) -> Result<()> {
    let hosts = filter_hosts(&rollback_args.hosts, &config.hosts)?;
    mgr::rollback(&hosts, flake, &config.global.secret_directory)
}

fn ssh(_args: &Args, ssh_args: &SshArgs, config: &Config) -> Result<()> {
    let hosts = filter_hosts(&ssh_args.hosts, &config.hosts)?;
    let command = ssh_args
        .command
        .as_ref()
        .map_or_else(|| [].as_slice(), |v| v.as_slice());
    let command = command.iter().map(|s| s.as_str()).collect::<Vec<_>>();
    mgr::ssh(&hosts, command.as_slice())
}

fn reboot(_args: &Args, reboot_args: &RebootArgs, config: &Config) -> Result<()> {
    let hosts = filter_hosts(&reboot_args.hosts, &config.hosts)?;
    mgr::reboot(&hosts)
}

fn system_info(args: &SystemInfoArgs, config: &Config) -> Result<()> {
    let hosts = filter_hosts(&args.hosts, &config.hosts)?;
    for host in hosts {
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
        println!("\n");
    }
    Ok(())
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
    create_or_update_lightning_certs(
        &config.global.secret_directory.join("lightning"),
        &config.hosts,
        &CertRenewPolicy::default(),
    )
    .context("failed to create or update lightning certificates")?;
    create_or_update_cockroachdb_certs(
        &config.global.secret_directory.join("cockroachdb"),
        &config.hosts,
        &CertRenewPolicy::default(),
    )
    .context("failed to create or update cockroachdb certificates")?;

    let flake = generate_nixos_flake(&config).context("failed to generate flake")?;

    let res = match args.action {
        Command::GenerateConfig(ref config_args) => {
            generate_config(&args, config_args, &config, &flake)
        }
        Command::Install(ref install_args) => install(&args, install_args, &config, &flake),
        Command::DryUpdate(ref dry_update_args) => {
            dry_update(&args, dry_update_args, &config, &flake)
        }
        Command::Update(ref update_args) => update(&args, update_args, &config, &flake),
        Command::Rollback(ref rollback_args) => rollback(&args, rollback_args, &config, &flake),
        Command::Ssh(ref ssh_args) => ssh(&args, ssh_args, &config),
        Command::Reboot(ref reboot_args) => reboot(&args, reboot_args, &config),
        Command::SystemInfo(ref args) => system_info(args, &config),
    };
    res.with_context(|| format!("kuutamo failed doing: {:?}", args.action))
}
