use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use log::warn;
use regex::Regex;
use reqwest::blocking::Client;
use serde::Serialize;
use serde_derive::Deserialize;
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::env::var;
use std::fs;
use std::hash::{Hash, Hasher};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use toml_example::TomlExample;
use url::Url;

use super::secrets::Secrets;
use super::NixosFlake;

/// IpV6String allows prefix only address format and normal ipv6 address
///
/// Some providers include the subnet in their address shown in the webinterface i.e. 2607:5300:203:5cdf::/64
/// This format is rejected by IpAddr in Rust and we store subnets in a different configuration option.
/// This struct detects such cashes in the kuutamo.toml file and converting it to 2607:5300:203:5cdf:: with a warning message, providing a more user-friendly experience.
type IpV6String = String;

trait AsIpAddr {
    /// Handle ipv6 subnet identifier and normalize to a valid ip address and a mask.
    fn normalize(&self) -> Result<(IpAddr, Option<u8>)>;
}

impl AsIpAddr for IpV6String {
    fn normalize(&self) -> Result<(IpAddr, Option<u8>)> {
        if let Some(idx) = self.find('/') {
            let mask = self
                .get(idx + 1..self.len())
                .map(|i| i.parse::<u8>())
                .with_context(|| {
                    format!("ipv6_address contains invalid subnet identifier: {self}")
                })?
                .ok();

            match self.get(0..idx) {
                Some(addr_str) if mask.is_some() => {
                    if let Ok(addr) = addr_str.parse::<IpAddr>() {
                        warn!("{self:} contains a ipv6 subnet identifier... will use {addr:} for ipv6_address and {:} for ipv6_cidr", mask.unwrap_or_default());
                        Ok((addr, mask))
                    } else {
                        Err(anyhow!("ipv6_address is not invalid"))
                    }
                }
                _ => Err(anyhow!("ipv6_address is not invalid")),
            }
        } else {
            Ok((self.parse::<IpAddr>()?, None))
        }
    }
}

#[derive(TomlExample, Debug, Deserialize)]
pub struct ConfigFile {
    #[serde(default)]
    #[toml_example(nesting)]
    global: Global,

    /// The default values of host will use if any corresponding value is not provided in following hosts
    #[serde(default)]
    #[toml_example(nesting)]
    host_defaults: HostDefaultConfig,

    /// The configuration for the host, if any field not provided will use from host_defaults
    /// For general use case, following fields is needed
    /// - one of network should be configured (ipv4 or ipv6)
    /// - the disk information of the node
    #[serde(default)]
    #[toml_example(nesting)]
    hosts: HashMap<String, HostConfig>,
}

fn default_secret_directory() -> PathBuf {
    PathBuf::from("secrets")
}

fn default_knd_flake() -> String {
    "github:kuutamolabs/lightning-knd".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct CockroachPeer {
    pub name: String,
    pub ipv4_address: Option<IpAddr>,
    pub ipv6_address: Option<IpAddr>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "warn")]
    Warn,
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "debug")]
    Debug,
    #[serde(rename = "trace")]
    Trace,
}

/// Kuutamo monitor
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Hash)]
pub struct KmonitorConfig {
    /// config for telegraf
    pub telegraf: Option<TelegrafConfig>,
    /// Promtail client endpoint with auth
    pub promtail: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Hash)]
pub struct TelegrafConfig {
    /// self host url for monitoring, None for kuutamo monitoring
    pub url: Option<Url>,
    /// username for kuutamo monitor
    pub username: String,
    /// password for kuutamo monitor
    pub password: String,
}

pub fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

#[derive(Debug, Default, Deserialize, TomlExample)]
struct HostDefaultConfig {
    /// The default Ipv4 gateway of all node
    #[serde(default)]
    #[toml_example(default = "192.168.0.254")]
    ipv4_gateway: Option<IpAddr>,
    /// The default Ipv4 CIDR for all node
    #[serde(default)]
    #[toml_example(default = 24)]
    ipv4_cidr: Option<u8>,
    /// The default Ipv6 gateway of all node
    #[serde(default)]
    ipv6_gateway: Option<IpAddr>,
    /// The default Ipv6 CIDR of all node
    #[serde(default)]
    ipv6_cidr: Option<u8>,

    /// The default ssh public keys of the user
    /// After installation the user could login as root with the corresponding ssh private key
    #[serde(default)]
    #[toml_example(default = [ "ssh-ed25519 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA...", ])]
    public_ssh_keys: Vec<String>,

    /// The default admin user for install,
    /// Please use `ubuntu` when you use OVH to install at first time,
    /// Ubuntu did not allow `root` login
    #[serde(default)]
    #[toml_example(default = "ubuntu")]
    install_ssh_user: Option<String>,

    /// Extra nixos module will deploy to the node
    #[serde(default)]
    #[toml_example(default = [ ])]
    extra_nixos_modules: Vec<String>,

    /// Default disk configure on all node
    #[serde(default)]
    #[toml_example(default = [ "/dev/vdb", ])]
    pub disks: Option<Vec<PathBuf>>,

    /// The default Token file for monitoring, default is "kuutamo-monitoring.token"
    /// Provide this if you have a different file
    #[serde(default)]
    #[toml_example(default = "kuutamo-monitoring.token")]
    kuutamo_monitoring_token_file: Option<PathBuf>,

    /// The default self monitoring server
    /// The url should implements [Prometheus's Remote Write API] (https://prometheus.io/docs/prometheus/latest/configuration/configuration/#remote_write).
    #[serde(default)]
    #[toml_example(default = "https://my.monitoring.server/api/v1/push")]
    self_monitoring_url: Option<Url>,
    /// The default http basic auth username to access self monitoring server
    #[serde(default)]
    self_monitoring_username: Option<String>,
    /// The default http basic auth password to access self monitoring server
    #[serde(default)]
    self_monitoring_password: Option<String>,

    /// The default push endpoint for the promtail client with auth to collect the journal logs for all nodes
    /// ex: https://<user_id>:<token>@<client hostname>/loki/api/vi/push
    #[serde(default)]
    promtail_client: Option<String>,
}

#[derive(Debug, Default, Deserialize, TomlExample)]
struct HostConfig {
    /// Ipv4 address of the node
    #[serde(default)]
    #[toml_example(default = "192.168.0.1")]
    ipv4_address: Option<IpAddr>,
    /// Ipv4 gateway of the node
    #[serde(default)]
    #[toml_example(default = "192.168.0.254")]
    ipv4_gateway: Option<IpAddr>,
    /// Ipv4 CIDR of the node
    #[serde(default)]
    #[toml_example(default = 24)]
    ipv4_cidr: Option<u8>,
    /// Nixos module will deploy to the node
    #[serde(default)]
    #[toml_example(default = "kld-node")]
    nixos_module: Option<String>,

    /// Mac address of the node
    #[toml_example(default = [ ])]
    #[serde(default)]
    #[toml_example(default = "00:0A:02:0B:03:0C")]
    pub mac_address: Option<String>,
    /// Ipv6 address of the node
    #[serde(default)]
    ipv6_address: Option<IpV6String>,
    /// Ipv6 gateway of the node
    #[serde(default)]
    ipv6_gateway: Option<IpAddr>,
    /// Ipv6 cidr of the node
    #[serde(default)]
    ipv6_cidr: Option<u8>,

    /// The ssh public keys of the user
    /// After installation the user could login as root with the corresponding ssh private key
    #[serde(default)]
    #[toml_example(skip)]
    #[toml_example(default = [ "ssh-ed25519 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA...", ])]
    public_ssh_keys: Vec<String>,

    /// Admin user for install,
    /// Please use `ubuntu` when you use OVH to install at first time,
    /// Ubuntu did not allow `root` login
    #[serde(default)]
    #[toml_example(default = "ubuntu")]
    install_ssh_user: Option<String>,

    /// Setup ssh host name for connection and host label on monitoring dashboard
    #[serde(default)]
    ssh_hostname: Option<String>,

    /// Disk configure on the node
    #[serde(default)]
    #[toml_example(default = [ "/dev/vdb", ])]
    pub disks: Option<Vec<PathBuf>>,

    #[serde(default)]
    pub bitcoind_disks: Option<Vec<PathBuf>>,

    /// String for node_alias, currently it only accept 32 chars ascii string for this field
    pub kld_node_alias: Option<String>,
    /// Set kld log level to `error`, `warn`, `info`, `debug`, `trace`
    #[serde(default)]
    #[toml_example(default = "info")]
    pub kld_log_level: Option<LogLevel>,

    /// Token file for monitoring, default is "kuutamo-monitoring.token"
    /// Provide this if you have a different file
    #[serde(default)]
    #[toml_example(default = "kuutamo-monitoring.token")]
    kuutamo_monitoring_token_file: Option<PathBuf>,
    /// Self monitoring server
    /// The url should implements [Prometheus's Remote Write API] (https://prometheus.io/docs/prometheus/latest/configuration/configuration/#remote_write).
    #[serde(default)]
    #[toml_example(default = "https://my.monitoring.server/api/v1/push")]
    self_monitoring_url: Option<Url>,
    /// The http basic auth username to access self monitoring server
    #[serde(default)]
    self_monitoring_username: Option<String>,
    /// The http basic auth password to access self monitoring server
    #[serde(default)]
    self_monitoring_password: Option<String>,

    /// The push endpoint for the promtail client with auth to collect the journal logs for the node
    /// ex: https://<user_id>:<token>@<client hostname>/loki/api/vi/push
    #[serde(default)]
    promtail_client: Option<String>,

    /// The communication port of kld
    #[toml_example(default = 2244)]
    #[serde(default)]
    kld_rest_api_port: Option<u16>,
    /// The ip addresses list will allow to communicate with kld, if empty, the kld-cli can only
    /// use on the node.
    #[serde(default)]
    #[toml_example(default = [])]
    kld_api_ip_access_list: Vec<IpAddr>,

    /// The interface to access network
    #[serde(default)]
    #[toml_example(default = "eth0")]
    network_interface: Option<String>,

    #[serde(flatten)]
    #[toml_example(skip)]
    others: BTreeMap<String, toml::Value>,
}

/// NixOS host configuration
#[derive(Debug, PartialEq, Eq, Clone, Serialize)]
pub struct Host {
    /// Name identifying the host
    pub name: String,

    /// NixOS module to use as a base for the host from the flake
    pub nixos_module: String,

    /// Extra NixOS modules to include in the system
    pub extra_nixos_modules: Vec<String>,

    /// Mac address of the public interface to use
    pub mac_address: Option<String>,

    /// Public ipv4 address of the host
    pub ipv4_address: Option<IpAddr>,
    /// Cidr of the public ipv4 address
    pub ipv4_cidr: Option<u8>,
    /// Public ipv4 gateway ip address
    pub ipv4_gateway: Option<IpAddr>,

    /// Public ipv6 address of the host
    pub ipv6_address: Option<IpAddr>,
    /// Cidr of the public ipv6 address
    pub ipv6_cidr: Option<u8>,
    /// Public ipv6 gateway address of the host
    pub ipv6_gateway: Option<IpAddr>,

    /// SSH Username used when connecting during installation
    pub install_ssh_user: String,

    /// SSH hostname used for connection and host label on monitoring dashboard
    pub ssh_hostname: String,

    /// Public ssh keys that will be added to the nixos configuration
    pub public_ssh_keys: Vec<String>,

    /// Block device paths to use for installing
    pub disks: Vec<PathBuf>,

    /// Block device paths to use for bitcoind's blockchain state
    pub bitcoind_disks: Vec<PathBuf>,

    /// CockroachDB nodes to connect to
    pub cockroach_peers: Vec<CockroachPeer>,

    /// alias of node in lightning
    pub kld_node_alias: Option<String>,
    /// Log level for kld service
    pub kld_log_level: Option<LogLevel>,

    /// Setup telegraf output auth for kuutamo monitor server
    #[serde(skip_serializing)]
    pub kmonitor_config: Option<KmonitorConfig>,

    /// The communication port of kld
    pub rest_api_port: Option<u16>,
    /// The ip addresses list will allow to communicate with kld
    pub api_ip_access_list: Vec<IpAddr>,

    /// Has monitoring server or not
    pub telegraf_has_monitoring: bool,

    /// Has client for journal logs or not
    pub promtail_has_client: bool,

    /// Hash for monitoring config
    pub monitor_config_hash: String,

    /// The interface of node to access the internet
    pub network_interface: Option<String>,

    /// Is the mnemonic provided by mgr
    pub kld_preset_mnemonic: Option<bool>,
}

impl Host {
    /// Returns prepared secrets directory for host
    pub fn secrets(&self, secrets_dir: &Path, access_tokens: &String) -> Result<Secrets> {
        let lightning = secrets_dir.join("lightning");
        let cockroachdb = secrets_dir.join("cockroachdb");
        let mnemonic = secrets_dir.join("mnemonic");
        let ssh = secrets_dir.join("ssh");

        let mut secret_files = vec![
            // for kld
            (
                PathBuf::from("/var/lib/secrets/kld/ca.pem"),
                fs::read(lightning.join("ca.pem")).context("failed to read ca.pem")?,
                0o600,
            ),
            (
                PathBuf::from("/var/lib/secrets/kld/kld.pem"),
                fs::read(lightning.join(format!("{}.pem", self.name)))
                    .context("failed to read kld.pem")?,
                0o600,
            ),
            (
                PathBuf::from("/var/lib/secrets/kld/kld.key"),
                fs::read(lightning.join(format!("{}.key", self.name)))
                    .context("failed to read kld.key")?,
                0o600,
            ),
            (
                PathBuf::from("/var/lib/secrets/kld/client.kld.crt"),
                fs::read(cockroachdb.join("client.kld.crt"))
                    .context("failed to read client.kld.crt")?,
                0o600,
            ),
            (
                PathBuf::from("/var/lib/secrets/kld/client.kld.key"),
                fs::read(cockroachdb.join("client.kld.key"))
                    .context("failed to read client.kld.key")?,
                0o600,
            ),
            // for cockroachdb
            (
                PathBuf::from("/var/lib/secrets/cockroachdb/ca.crt"),
                fs::read(cockroachdb.join("ca.crt")).context("failed to read ca.crt")?,
                0o600,
            ),
            (
                PathBuf::from("/var/lib/secrets/cockroachdb/client.root.crt"),
                fs::read(cockroachdb.join("client.root.crt"))
                    .context("failed to read client.root.crt")?,
                0o600,
            ),
            (
                PathBuf::from("/var/lib/secrets/cockroachdb/client.root.key"),
                fs::read(cockroachdb.join("client.root.key"))
                    .context("failed to read client.root.key")?,
                0o600,
            ),
            (
                PathBuf::from("/var/lib/secrets/cockroachdb/node.crt"),
                fs::read(cockroachdb.join(format!("{}.node.crt", self.name)))
                    .context("failed to read node.crt")?,
                0o600,
            ),
            (
                PathBuf::from("/var/lib/secrets/cockroachdb/node.key"),
                fs::read(cockroachdb.join(format!("{}.node.key", self.name)))
                    .context("failed to read node.key")?,
                0o600,
            ),
            (
                PathBuf::from("/root/.ssh/id_ed25519"),
                fs::read(ssh.join("id_ed25519")).context("failed to read deploy key")?,
                0o600,
            ),
            (
                PathBuf::from("/root/.ssh/id_ed25519.pub"),
                fs::read(ssh.join("id_ed25519.pub")).context("failed to read deploy pub key")?,
                0o644,
            ),
            // sshd server key
            (
                PathBuf::from("/var/lib/secrets/sshd_key"),
                fs::read(secrets_dir.join("sshd").join(&self.name))
                    .context("failed to read sshd server key")?,
                0o600,
            ),
            (
                PathBuf::from("/var/lib/secrets/access-tokens"),
                format!("ACCESS_TOKENS={access_tokens:}").as_bytes().into(),
                0o600,
            ),
            (
                PathBuf::from("/var/lib/secrets/disk_encryption_key"),
                fs::read(secrets_dir.join("disk_encryption_key"))
                    .context("failed to read disk_encrypted_key")?,
                0o600,
            ),
        ];
        if mnemonic.exists() {
            secret_files.push((
                PathBuf::from("/var/lib/secrets/mnemonic"),
                fs::read(mnemonic).context("failed to read mnemonic")?,
                0o600,
            ))
        }
        if let Some(KmonitorConfig { telegraf, promtail }) = &self.kmonitor_config {
            if let Some(TelegrafConfig {
                url,
                username,
                password,
            }) = telegraf
            {
                secret_files.push((
                    PathBuf::from("/var/lib/secrets/telegraf"),
                    format!("MONITORING_URL={}\nMONITORING_USERNAME={username}\nMONITORING_PASSWORD={password}", url.as_ref().map(|u|u.to_string()).unwrap_or("https://mimir.monitoring-00-cluster.kuutamo.computer/api/v1/push".to_string())).as_bytes().into(),
                    0o600
                ));
            }
            if let Some(client) = promtail {
                secret_files.push((
                    PathBuf::from("/var/lib/secrets/promtail"),
                    format!("CLIENT_URL={client}").as_bytes().into(),
                    0o600,
                ));
            }
        }

        Secrets::new(secret_files.iter()).context("failed to prepare uploading secrets")
    }
    /// The hostname to which we will deploy
    pub fn deploy_ssh_target(&self) -> String {
        format!("root@{}", self.ssh_hostname)
    }
    /// The hostname to which we will deploy
    pub fn flake_uri(&self, flake: &NixosFlake) -> String {
        format!("{}#{}", flake.path().display(), self.name)
    }
}

/// Global configuration affecting all hosts
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Default, TomlExample)]
pub struct Global {
    /// Flake url for your deployment config
    /// Please refer https://github.com/kuutamolabs/deployment-example
    #[toml_example(default = "github:kuutamolabs/deployment-example")]
    pub deployment_flake: String,

    /// Tokens for access the deployment flake and the dependencies thereof
    /// Please make sure it is never exipired,
    /// because we can not update the token after deploy
    #[toml_example(default = "github.com=ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx")]
    pub access_tokens: String,

    /// Flake url for KND
    #[serde(default = "default_knd_flake")]
    #[toml_example(default = "github:kuutamolabs/lightning-knd")]
    pub knd_flake: String,

    /// Directory where the secrets are stored i.e. certificates
    #[serde(default = "default_secret_directory")]
    #[toml_example(default = "secrets")]
    pub secret_directory: PathBuf,
}

fn validate_global(global: &Global, working_directory: &Path) -> Result<Global> {
    let mut global = global.clone();
    if global.secret_directory.is_relative() {
        global.secret_directory = working_directory.join(global.secret_directory);
    };
    if let Ok(output) = Command::new("nix")
        .args([
            "flake",
            "show",
            "--refresh",
            "--access-tokens",
            &global.access_tokens,
            &global.deployment_flake,
        ])
        .output()
    {
        if !output.status.success() && var("FLAKE_CHECK").is_err() {
            bail!(
                "deployment flake {} is not accessible, please check your access token or network connection",
                global.deployment_flake
            );
        }
    }
    Ok(global)
}

fn validate_host(
    name: &str,
    host: &HostConfig,
    default: &HostDefaultConfig,
    preset_mnemonic: bool,
) -> Result<Host> {
    if !host.others.is_empty() {
        bail!(
            "{} are not allowed fields",
            host.others
                .clone()
                .into_keys()
                .collect::<Vec<String>>()
                .join(", ")
        );
    }

    let name = name.to_string();

    if name.is_empty() || name.len() > 63 {
        bail!(
            "a host's name must be between 1 and 63 characters long, got: '{}'",
            name
        );
    }
    let hostname_regex = Regex::new(r"^[a-z0-9][a-z0-9\-]{0,62}$").unwrap();
    if !hostname_regex.is_match(&name) {
        bail!("a host's name must only contain letters from a to z, the digits from 0 to 9, and the hyphen (-). But not starting with a hyphen. got: '{}'", name);
    }
    let mac_address = if let Some(ref a) = &host.mac_address {
        let mac_address_regex = Regex::new(r"^([0-9A-Fa-f]{2}[:-]){5}([0-9A-Fa-f]{2})$").unwrap();
        if !mac_address_regex.is_match(a) {
            bail!("mac address does match a valid format: {} (valid example value: 02:42:34:d1:18:7a)", a);
        }
        Some(a.clone())
    } else {
        None
    };

    let ipv4_address = if let Some(address) = host.ipv4_address {
        if !address.is_ipv4() {
            bail!("ipv4_address provided for hosts.{name} is not an ipv4 address: {address}");
        }
        // FIXME: this is currently an unstable feature
        //if address.is_global() {
        //    warn!("ipv4_address provided for hosts.{} is not a public ipv4 address: {}.", name, address);
        //}
        Some(address)
    } else {
        None
    };

    let ipv4_cidr = if let Some(cidr) = host.ipv4_cidr.or(default.ipv4_cidr) {
        if !(0..32_u8).contains(&cidr) {
            bail!("ipv4_cidr for hosts.{name} is not between 0 and 32: {cidr}")
        }
        Some(cidr)
    } else {
        None
    };

    let nixos_module = host
        .nixos_module
        .as_deref()
        .with_context(|| format!("no nixos_module provided for hosts.{name}"))?
        .to_string();

    let mut extra_nixos_modules = vec![];
    extra_nixos_modules.extend_from_slice(&default.extra_nixos_modules);

    let ipv4_gateway = host.ipv4_gateway.or(default.ipv4_gateway);

    let ipv6_cidr = host.ipv6_cidr.or(default.ipv6_cidr);

    let ipv6_gateway = host.ipv6_gateway.or(default.ipv6_gateway);

    let (ipv6_address, mask) = if let Some(ipv6_address) = host.ipv6_address.as_ref() {
        let (ipv6_address, mask) = ipv6_address
            .normalize()
            .with_context(|| format!("ipv6_address provided for host.{name:} is not valid"))?;
        if !ipv6_address.is_ipv6() {
            bail!("value provided in ipv6_address for hosts.{name} is not an ipv6 address: {ipv6_address}");
        }

        if let Some(ipv6_cidr) = ipv6_cidr {
            if !(0..128_u8).contains(&ipv6_cidr) {
                bail!("ipv6_cidr for hosts.{name} is not between 0 and 128: {ipv6_cidr}")
            }
        } else if mask.is_none() {
            bail!("no ipv6_cidr provided for hosts.{name}");
        }

        if ipv6_gateway.is_none() {
            bail!("no ipv6_gateway provided for hosts.{name}")
        }

        // FIXME: this is currently an unstable feature
        //if ipv6_address.is_global() {
        //    warn!("ipv6_address provided for hosts.{} is not a public ipv6 address: {}.", name, ipv6_address);
        //}

        (Some(ipv6_address), mask)
    } else {
        (None, None)
    };

    let address = ipv4_address
        .or(ipv6_address)
        .with_context(|| format!("no ipv4_address or ipv6_address provided for hosts.{name}"))?;

    if ipv4_gateway.is_none() && ipv6_gateway.is_none() {
        bail!("no ipv4_gateway or ipv6_gateway provided for hosts.{name}");
    }

    let ssh_hostname = host
        .ssh_hostname
        .as_ref()
        .cloned()
        .unwrap_or_else(|| address.to_string());

    let install_ssh_user = host
        .install_ssh_user
        .as_ref()
        .or(default.install_ssh_user.as_ref())
        .cloned()
        .unwrap_or_else(|| String::from("root"));

    let mut public_ssh_keys = vec![];
    public_ssh_keys.extend_from_slice(&host.public_ssh_keys);
    public_ssh_keys.extend_from_slice(&default.public_ssh_keys);
    if public_ssh_keys.is_empty() {
        bail!("no public_ssh_keys provided for hosts.{name}");
    }

    let default_disks = vec![PathBuf::from("/dev/nvme0n1"), PathBuf::from("/dev/nvme1n1")];
    let disks = host
        .disks
        .as_ref()
        .or(default.disks.as_ref())
        .unwrap_or(&default_disks)
        .to_vec();

    if disks.is_empty() {
        bail!("no disks specified for hosts.{name}");
    }

    let default_bitcoind_disks = vec![];

    let bitcoind_disks = host
        .bitcoind_disks
        .as_ref()
        .unwrap_or(&default_bitcoind_disks)
        .to_vec();

    let telegraf_config = match (
        &host
            .self_monitoring_url
            .as_ref()
            .or(default.self_monitoring_url.as_ref()),
        &host
            .self_monitoring_username
            .as_ref()
            .or(default.self_monitoring_username.as_ref()),
        &host
            .self_monitoring_password
            .as_ref()
            .or(default.self_monitoring_password.as_ref()),
        fs::read_to_string(
            host.kuutamo_monitoring_token_file
                .as_ref()
                .or(default.kuutamo_monitoring_token_file.as_ref())
                .unwrap_or(&PathBuf::from("kuutamo-monitoring.token")),
        )
        .ok()
        .map(|s| s.trim().into())
        .and_then(|t| decode_token(t).ok()),
    ) {
        (url, Some(username), Some(password), _) if url.is_some() => Some(TelegrafConfig {
            url: url.cloned(),
            username: username.to_string(),
            password: password.to_string(),
        }),
        (url, _, _, Some((user_id, password))) if url.is_some() => Some(TelegrafConfig {
            url: url.cloned(),
            username: user_id,
            password,
        }),
        (None, _, _, Some((user_id, password))) => {
            try_verify_kuutamo_monitoring_config(user_id, password)
        }
        _ => {
            eprintln!("auth information for monitoring is insufficient, will not set up monitoring when deploying");
            None
        }
    };
    let kmonitor_config = if telegraf_config.is_some()
        || host.promtail_client.is_some()
        || default.promtail_client.is_some()
    {
        Some(KmonitorConfig {
            telegraf: telegraf_config,
            promtail: host
                .promtail_client
                .clone()
                .or(default.promtail_client.clone()),
        })
    } else {
        None
    };

    let telegraf_has_monitoring = kmonitor_config
        .as_ref()
        .map(|c| c.telegraf.is_some())
        .unwrap_or_default();
    let promtail_has_client = kmonitor_config
        .as_ref()
        .map(|c| c.promtail.is_some())
        .unwrap_or_default();
    let monitor_config_hash = calculate_hash(&kmonitor_config).to_string();

    if let Some(alias) = &host.kld_node_alias {
        // none ascii word will take more than one bytes and we can not validate it with len()
        if !alias.is_ascii() {
            bail!("currently alias should be ascii");
        }
        if alias.len() > 32 {
            bail!("alias should be 32 bytes");
        }
    }

    Ok(Host {
        name,
        nixos_module,
        extra_nixos_modules,
        install_ssh_user,
        ssh_hostname,
        mac_address,
        ipv4_address,
        ipv4_cidr,
        ipv4_gateway,
        ipv6_address,
        ipv6_cidr: mask.or(ipv6_cidr),
        ipv6_gateway,
        public_ssh_keys,
        disks,
        bitcoind_disks,
        cockroach_peers: vec![],
        kld_log_level: host.kld_log_level.to_owned(),
        kmonitor_config,
        telegraf_has_monitoring,
        promtail_has_client,
        monitor_config_hash,
        kld_node_alias: host.kld_node_alias.to_owned(),
        api_ip_access_list: host.kld_api_ip_access_list.to_owned(),
        rest_api_port: host.kld_rest_api_port,
        network_interface: host.network_interface.to_owned(),
        kld_preset_mnemonic: Some(preset_mnemonic),
    })
}

/// Try to access kuutamo monitoring , if auth is invalid the config will drop
fn try_verify_kuutamo_monitoring_config(
    user_id: String,
    password: String,
) -> Option<TelegrafConfig> {
    let client = Client::new();
    let username = format!("kld-{}", user_id);
    if let Ok(r) = client
        .get("https://mimir.monitoring-00-cluster.kuutamo.computer")
        .basic_auth(&username, Some(&password))
        .send()
    {
        if r.status() == reqwest::StatusCode::UNAUTHORIZED {
            eprintln!("token for kuutamo monitoring.token is invalid, please check, else the monitor will not work after deploy");
            return None;
        }
    } else {
        eprintln!("Could not validate kuutamo-monitoring.token (network issue)");
    }

    Some(TelegrafConfig {
        url: None,
        username,
        password,
    })
}

/// Validated configuration
pub struct Config {
    /// Hosts as defined in the configuration
    pub hosts: BTreeMap<String, Host>,
    /// Configuration affecting all hosts
    pub global: Global,
}

/// Parse toml configuration
pub fn parse_config(
    content: &str,
    working_directory: &Path,
    preset_mnemonic: bool,
) -> Result<Config> {
    let config: ConfigFile = toml::from_str(content)?;

    let mut hosts = config
        .hosts
        .iter()
        .map(|(name, host)| {
            Ok((
                name.to_string(),
                validate_host(name, host, &config.host_defaults, preset_mnemonic)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let cockroach_peers = hosts
        .iter()
        .map(|(name, host)| CockroachPeer {
            name: name.to_string(),
            ipv4_address: host.ipv4_address,
            ipv6_address: host.ipv6_address,
        })
        .collect::<Vec<_>>();
    for host in hosts.values_mut() {
        host.cockroach_peers = cockroach_peers.clone();
    }
    let kld_nodes = hosts
        .iter()
        .filter(|(_, host)| host.nixos_module == "kld-node")
        .count();
    if kld_nodes != 1 {
        bail!("Exactly one kld-node is required, found {}", kld_nodes);
    }
    let cockroach_nodes = hosts
        .iter()
        .filter(|(_, host)| host.nixos_module == "cockroachdb-node")
        .count();
    if cockroach_nodes != 0 && cockroach_nodes < 2 {
        bail!(
            "Either zero or two cockroach-nodes are required, found {}",
            cockroach_nodes
        );
    }

    let global = validate_global(&config.global, working_directory)?;

    Ok(Config { hosts, global })
}

/// Load configuration from path
pub fn load_configuration(path: &Path, preset_mnemonic: bool) -> Result<Config> {
    let content = fs::read_to_string(path).context("Cannot read file")?;
    let working_directory = path.parent().with_context(|| {
        format!(
            "Cannot determine working directory from path: {}",
            path.display()
        )
    })?;
    parse_config(&content, working_directory, preset_mnemonic)
}

fn decode_token(s: String) -> Result<(String, String)> {
    let binding =
        general_purpose::STANDARD_NO_PAD.decode(s.trim_matches(|c| c == '=' || c == '\n'))?;
    let decode_str = std::str::from_utf8(&binding)?;
    decode_str
        .split_once(':')
        .map(|(u, p)| (u.trim().to_string(), p.trim().to_string()))
        .ok_or(anyhow!("token should be `username: password` pair"))
}

#[cfg(test)]
pub(crate) const TEST_CONFIG: &str = r#"
[global]
knd_flake = "github:kuutamolabs/lightning-knd"
deployment_flake = "github:kuutamolabs/test-env-one"
access_tokens = "github.com=ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"

[host_defaults]
public_ssh_keys = [
  '''ssh-ed25519 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA foobar'''
]
ipv4_cidr = 24
ipv6_cidr = 48
ipv4_gateway = "199.127.64.1"
ipv6_gateway = "2605:9880:400::1"

[hosts]
[hosts.kld-00]
nixos_module = "kld-node"
ipv4_address = "199.127.64.2"
ipv6_address = "2605:9880:400::2"
ipv6_cidr = 48

[hosts.db-00]
nixos_module = "cockroachdb-node"
ipv4_address = "199.127.64.3"
ipv6_address = "2605:9880:400::3"

[hosts.db-01]
nixos_module = "cockroachdb-node"
ipv4_address = "199.127.64.4"
ipv6_address = "2605:9880:400::4"
"#;

#[test]
pub fn test_parse_config() -> Result<()> {
    use std::str::FromStr;

    let config = parse_config(TEST_CONFIG, Path::new("/"), false)?;
    assert_eq!(config.global.knd_flake, "github:kuutamolabs/lightning-knd");
    assert_eq!(
        config.global.deployment_flake,
        "github:kuutamolabs/test-env-one"
    );

    let hosts = &config.hosts;
    assert_eq!(hosts.len(), 3);
    assert_eq!(
        hosts["kld-00"]
            .ipv4_address
            .context("missing ipv4_address")?,
        IpAddr::from_str("199.127.64.2").unwrap()
    );
    assert_eq!(hosts["kld-00"].ipv4_cidr.context("missing ipv4_cidr")?, 24);
    assert_eq!(
        hosts["db-00"]
            .ipv4_gateway
            .context("missing ipv4_gateway")?,
        IpAddr::from_str("199.127.64.1").unwrap()
    );
    assert_eq!(
        hosts["db-00"].ipv6_address,
        IpAddr::from_str("2605:9880:400::3").ok()
    );
    assert_eq!(hosts["kld-00"].ipv6_cidr, Some(48));
    assert_eq!(
        hosts["kld-00"].ipv6_gateway,
        IpAddr::from_str("2605:9880:400::1").ok()
    );

    parse_config(TEST_CONFIG, Path::new("/"), false)?;

    Ok(())
}

#[test]
pub fn test_parse_config_with_redundant_filds() {
    let parse_result = parse_config(
        &format!("{}\nredundant = 111", TEST_CONFIG),
        Path::new("/"),
        false,
    );
    assert!(parse_result.is_err());
}

#[test]
fn test_valid_ip_string_for_ipv6() {
    let ip: IpV6String = "2607:5300:203:5cdf::".into();
    assert_eq!(ip.normalize().unwrap().1, None);

    let subnet_identifire: IpV6String = "2607:5300:203:5cdf::/64".into();
    assert_eq!(
        subnet_identifire.normalize().unwrap().0,
        ip.normalize().unwrap().0
    );
    assert_eq!(subnet_identifire.normalize().unwrap().1, Some(64));
}

#[test]
fn test_invalid_string_for_ipv6() {
    let mut invalid_str: IpV6String = "2607:5300:203:7cdf::/".into();
    assert!(invalid_str.normalize().is_err());

    invalid_str = "/2607:5300:203:7cdf::".into();
    assert!(invalid_str.normalize().is_err());
}

#[test]
fn test_validate_host() -> Result<()> {
    let mut config = HostConfig {
        ipv4_address: Some(
            "192.168.0.1"
                .parse::<IpAddr>()
                .context("Invalid IP address")?,
        ),
        nixos_module: Some("kld-node".to_string()),
        ipv4_cidr: Some(0),
        ipv4_gateway: Some(
            "192.168.255.255"
                .parse::<IpAddr>()
                .context("Invalid IP address")?,
        ),
        ipv6_address: None,
        ipv6_gateway: None,
        ipv6_cidr: None,
        public_ssh_keys: vec!["".to_string()],
        ..Default::default()
    };
    assert_eq!(
        validate_host("ipv4-only", &config, &HostDefaultConfig::default(), false).unwrap(),
        Host {
            name: "ipv4-only".to_string(),
            nixos_module: "kld-node".to_string(),
            extra_nixos_modules: Vec::new(),
            mac_address: None,
            ipv4_address: Some(
                "192.168.0.1"
                    .parse::<IpAddr>()
                    .context("Invalid IP address")?
            ),
            ipv4_cidr: Some(0),
            ipv4_gateway: Some(
                "192.168.255.255"
                    .parse::<IpAddr>()
                    .context("Invalid IP address")?
            ),
            ipv6_address: None,
            ipv6_cidr: None,
            ipv6_gateway: None,
            install_ssh_user: "root".to_string(),
            ssh_hostname: "192.168.0.1".to_string(),
            public_ssh_keys: vec!["".to_string()],
            disks: vec!["/dev/nvme0n1".into(), "/dev/nvme1n1".into()],
            cockroach_peers: vec![],
            bitcoind_disks: vec![],
            kld_node_alias: None,
            kld_log_level: None,
            kmonitor_config: None,
            telegraf_has_monitoring: false,
            promtail_has_client: false,
            monitor_config_hash: "13646096770106105413".to_string(),
            api_ip_access_list: Vec::new(),
            rest_api_port: None,
            network_interface: None,
            kld_preset_mnemonic: Some(false),
        }
    );

    // If `ipv6_address` is provided, the `ipv6_gateway` and `ipv6_cidr` should be provided too,
    // else the error will raise
    config.ipv6_address = Some("2607:5300:203:6cdf::".into());
    assert!(validate_host("ipv4-only", &config, &HostDefaultConfig::default(), false).is_err());

    config.ipv6_gateway = Some(
        "2607:5300:0203:6cff:00ff:00ff:00ff:00ff"
            .parse::<IpAddr>()
            .unwrap(),
    );
    assert!(validate_host("ipv4-only", &config, &HostDefaultConfig::default(), false).is_err());

    // The `ipv6_cidr` could be provided by subnet in address field
    config.ipv6_address = Some("2607:5300:203:6cdf::/64".into());
    assert!(validate_host("ipv4-only", &config, &HostDefaultConfig::default(), false).is_ok());

    Ok(())
}
