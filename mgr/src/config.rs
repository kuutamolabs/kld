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
use std::fs;
use std::hash::{Hash, Hasher};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
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
    /// Handle ipv6 subnet identifier and normalize to a valide ip address and a mask.
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
    host_defaults: HostConfig,

    /// The configure for host, if any field not provided will use from host_defaults
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

fn default_flake() -> String {
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
    /// self host url for monitoring, None for kuutamo monitoring
    pub url: Option<Url>,
    /// username for kuutamo monitor
    pub username: String,
    /// password for kuutamo monitor
    pub password: String,
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
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
    /// Extra nixos module will deploy to the node
    #[serde(default)]
    #[toml_example(default = [ ])]
    extra_nixos_modules: Vec<String>,

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
    #[toml_example(default = [ "ssh-ed25519 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA...", ])]
    public_ssh_keys: Vec<String>,

    /// The ssh key for users
    /// After installation these user could login with their name with the corresponding ssh private key
    #[serde(default)]
    #[toml_example(default = [ "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIE44HxTp1mXzBfAgc66edFb7PxOmh2SpihdhoWUYxwYl username", ])]
    user_ssh_keys: Vec<String>,

    /// Admin user for install,
    /// Please use `ubuntu` when you use OVH to install at first time,
    /// Ubuntu did not allow `root` login
    #[serde(default)]
    #[toml_example(default = "ubuntu")]
    install_ssh_user: Option<String>,

    /// The user for login and execute commands, after installation
    #[serde(default)]
    #[toml_example(default = "kuutamo")]
    run_as_user: Option<String>,

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

    /// The communication port of kld
    #[toml_example(default = 2244)]
    #[serde(default)]
    kld_rest_api_port: Option<u16>,
    /// The ip addresses list will allow to communicate with kld, if empty, the kld-cli can only
    /// use on the node.
    #[serde(default)]
    #[toml_example(default = [])]
    kld_api_ip_access_list: Vec<IpAddr>,

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

    /// SSH Username used when executing commands after installation
    pub run_as_user: String,

    /// SSH hostname used for connection and host label on monitoring dashboard
    pub ssh_hostname: String,

    /// Public ssh keys that will be added to the nixos configuration
    pub public_ssh_keys: Vec<String>,

    /// The user with ssh key
    pub users: HashMap<String, String>,

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

    /// Has monitoring server or not
    pub telegraf_has_monitoring: bool,

    /// Hash for monitoring config
    pub telegraf_config_hash: String,

    /// The communication port of kld
    pub rest_api_port: Option<u16>,
    /// The ip addresses list will allow to communicate with kld
    pub api_ip_access_list: Vec<IpAddr>,

    /// Is the mnemonic provided by mgr
    pub kld_preset_mnemonic: Option<bool>,
}

impl Host {
    /// Returns prepared secrets directory for host
    pub fn secrets(&self, secrets_dir: &Path) -> Result<Secrets> {
        let lightning = secrets_dir.join("lightning");
        let cockroachdb = secrets_dir.join("cockroachdb");
        let mnemonic = secrets_dir.join("mnemonic");

        let mut secret_files = vec![
            // for kld
            (
                PathBuf::from("/var/lib/secrets/kld/ca.pem"),
                fs::read_to_string(lightning.join("ca.pem")).context("failed to read ca.pem")?,
            ),
            (
                PathBuf::from("/var/lib/secrets/kld/kld.pem"),
                fs::read_to_string(lightning.join(format!("{}.pem", self.name)))
                    .context("failed to read kld.pem")?,
            ),
            (
                PathBuf::from("/var/lib/secrets/kld/kld.key"),
                fs::read_to_string(lightning.join(format!("{}.key", self.name)))
                    .context("failed to read kld.key")?,
            ),
            (
                PathBuf::from("/var/lib/secrets/kld/client.kld.crt"),
                fs::read_to_string(cockroachdb.join("client.kld.crt"))
                    .context("failed to read client.kld.crt")?,
            ),
            (
                PathBuf::from("/var/lib/secrets/kld/client.kld.key"),
                fs::read_to_string(cockroachdb.join("client.kld.key"))
                    .context("failed to read client.kld.key")?,
            ),
            // for cockroachdb
            (
                PathBuf::from("/var/lib/secrets/cockroachdb/ca.crt"),
                fs::read_to_string(cockroachdb.join("ca.crt")).context("failed to read ca.crt")?,
            ),
            (
                PathBuf::from("/var/lib/secrets/cockroachdb/client.root.crt"),
                fs::read_to_string(cockroachdb.join("client.root.crt"))
                    .context("failed to read client.root.crt")?,
            ),
            (
                PathBuf::from("/var/lib/secrets/cockroachdb/client.root.key"),
                fs::read_to_string(cockroachdb.join("client.root.key"))
                    .context("failed to read client.root.key")?,
            ),
            (
                PathBuf::from("/var/lib/secrets/cockroachdb/node.crt"),
                fs::read_to_string(cockroachdb.join(format!("{}.node.crt", self.name)))
                    .context("failed to read node.crt")?,
            ),
            (
                PathBuf::from("/var/lib/secrets/cockroachdb/node.key"),
                fs::read_to_string(cockroachdb.join(format!("{}.node.key", self.name)))
                    .context("failed to read node.key")?,
            ),
        ];
        if mnemonic.exists() {
            secret_files.push((
                PathBuf::from("/var/lib/secrets/mnemonic"),
                fs::read_to_string(mnemonic).context("failed to read mnemonic")?,
            ))
        }
        if let Some(KmonitorConfig {
            url,
            username,
            password,
        }) = &self.kmonitor_config
        {
            secret_files.push((
                PathBuf::from("/var/lib/secrets/telegraf"),
                format!("MONITORING_URL={}\nMONITORING_USERNAME={username}\nMONITORING_PASSWORD={password}", url.as_ref().map(|u|u.to_string()).unwrap_or("https://mimir.monitoring-00-cluster.kuutamo.computer/api/v1/push".to_string()))
            ));
        }

        Secrets::new(secret_files.iter()).context("failed to prepare uploading secrets")
    }
    /// The hostname to which we will deploy
    pub fn deploy_ssh_target(&self) -> String {
        format!("{}@{}", self.install_ssh_user, self.ssh_hostname)
    }

    /// The hostname to which we will perform command
    pub fn execute_ssh_target(&self) -> String {
        format!("{}@{}", self.run_as_user, self.ssh_hostname)
    }
    /// The hostname to which we will deploy
    pub fn flake_uri(&self, flake: &NixosFlake) -> String {
        format!("{}#{}", flake.path().display(), self.name)
    }
}

/// Global configuration affecting all hosts
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Default, TomlExample)]
pub struct Global {
    /// Flake url where the nixos configuration is
    #[serde(default = "default_flake")]
    #[toml_example(default = "github:kuutamolabs/lightning-knd")]
    pub flake: String,

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
    Ok(global)
}

fn validate_host(
    name: &str,
    host: &HostConfig,
    default: &HostConfig,
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
    extra_nixos_modules.extend_from_slice(&host.extra_nixos_modules);
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
        .or(default.ssh_hostname.as_ref())
        .cloned()
        .unwrap_or_else(|| address.to_string());

    let install_ssh_user = host
        .install_ssh_user
        .as_ref()
        .or(default.install_ssh_user.as_ref())
        .cloned()
        .unwrap_or_else(|| String::from("root"));

    let run_as_user = host
        .run_as_user
        .as_ref()
        .or(default.run_as_user.as_ref())
        .cloned()
        .unwrap_or_else(|| install_ssh_user.clone());

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
        .or(default.bitcoind_disks.as_ref())
        .unwrap_or(&default_bitcoind_disks)
        .to_vec();

    let kmonitor_config = match (
        &host.self_monitoring_url,
        &host.self_monitoring_username,
        &host.self_monitoring_password,
        fs::read_to_string(
            host.kuutamo_monitoring_token_file
                .as_ref()
                .unwrap_or(&PathBuf::from("kuutamo-monitoring.token")),
        )
        .ok()
        .map(|s| s.trim().into())
        .and_then(|t| decode_token(t).ok()),
    ) {
        (url, Some(username), Some(password), _) if url.is_some() => Some(KmonitorConfig {
            url: url.clone(),
            username: username.to_string(),
            password: password.to_string(),
        }),
        (url, _, _, Some((user_id, password))) if url.is_some() => Some(KmonitorConfig {
            url: url.clone(),
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

    let telegraf_has_monitoring = kmonitor_config.is_some();
    let telegraf_config_hash = calculate_hash(&kmonitor_config).to_string();

    if let Some(alias) = &host.kld_node_alias {
        // none ascii word will take more than one bytes and we can not validate it with len()
        if !alias.is_ascii() {
            bail!("currently alias should be ascii");
        }
        if alias.len() > 32 {
            bail!("alias should be 32 bytes");
        }
    }
    let mut users = HashMap::new();

    for ssh_key in default.user_ssh_keys.iter() {
        let user_name = ssh_key.split(' ').nth(2).map(|name_with_host|{
                if let Some((user_name, _)) = name_with_host.split_once('@') {
                    user_name
                } else {
                    name_with_host
                }
            }).ok_or(anyhow!("user_ssh_keys in [host_defaults] should have `username` or `username@hostname` in the end"))?;
        if user_name == "root" {
            bail!("Creating a root user is not allowed");
        }
        users.insert(user_name.into(), ssh_key.into());
    }

    for ssh_key in host.user_ssh_keys.iter() {
        let user_name = ssh_key
            .split(' ')
            .nth(2)
            .map(|name_with_host| {
                if let Some((user_name, _)) = name_with_host.split_once('@') {
                    user_name
                } else {
                    name_with_host
                }
            })
            .ok_or(anyhow!(
                "user_ssh_keys in [{}] should have `username` or `username@hostname` in the end",
                name
            ))?;
        if user_name == "root" {
            bail!("Creating a root user is not allowed");
        }
        if users.insert(user_name.into(), ssh_key.into()).is_some() {
            warn!("{user_name} duplicated in {name}, it will use defined in [{name}] not in [host_defaults]");
        }
    }

    if run_as_user != install_ssh_user && !users.contains_key(&run_as_user) {
        warn!("The run_as_user is not in the list of user_ssh_keys, you may not login or control the node after installation")
    }

    Ok(Host {
        name,
        nixos_module,
        extra_nixos_modules,
        install_ssh_user,
        run_as_user,
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
        telegraf_config_hash,
        kld_node_alias: host.kld_node_alias.to_owned(),
        api_ip_access_list: host.kld_api_ip_access_list.to_owned(),
        rest_api_port: host.kld_rest_api_port,
        kld_preset_mnemonic: Some(preset_mnemonic),
        users,
    })
}

/// Try to access kuutamo monitoring , if auth is invalid the config will drop
fn try_verify_kuutamo_monitoring_config(
    user_id: String,
    password: String,
) -> Option<KmonitorConfig> {
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

    Some(KmonitorConfig {
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
flake = "github:myfork/lightning-knd"

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
    assert_eq!(config.global.flake, "github:myfork/lightning-knd");

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
        user_ssh_keys: vec!["ssh-ed25519 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA kuutamo@kuutamo.co".to_string()],
        ..Default::default()
    };
    let mut users = HashMap::new();
    users.insert("kuutamo".into(), "ssh-ed25519 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA kuutamo@kuutamo.co".into());
    assert_eq!(
        validate_host("ipv4-only", &config, &HostConfig::default(), false).unwrap(),
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
            run_as_user: "root".to_string(),
            ssh_hostname: "192.168.0.1".to_string(),
            public_ssh_keys: vec!["".to_string()],
            disks: vec!["/dev/nvme0n1".into(), "/dev/nvme1n1".into()],
            cockroach_peers: vec![],
            bitcoind_disks: vec![],
            kld_node_alias: None,
            kld_log_level: None,
            kmonitor_config: None,
            telegraf_has_monitoring: false,
            telegraf_config_hash: "13646096770106105413".to_string(),
            api_ip_access_list: Vec::new(),
            rest_api_port: None,
            kld_preset_mnemonic: Some(false),
            users,
        }
    );

    // If `ipv6_address` is provied, the `ipv6_gateway` and `ipv6_cidr` should be provided too,
    // else the error will raise
    config.ipv6_address = Some("2607:5300:203:6cdf::".into());
    assert!(validate_host("ipv4-only", &config, &HostConfig::default(), false).is_err());

    config.ipv6_gateway = Some(
        "2607:5300:0203:6cff:00ff:00ff:00ff:00ff"
            .parse::<IpAddr>()
            .unwrap(),
    );
    assert!(validate_host("ipv4-only", &config, &HostConfig::default(), false).is_err());

    // The `ipv6_cidr` could be provided by subnet in address field
    config.ipv6_address = Some("2607:5300:203:6cdf::/64".into());
    assert!(validate_host("ipv4-only", &config, &HostConfig::default(), false).is_ok());

    Ok(())
}
