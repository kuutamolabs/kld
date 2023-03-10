use anyhow::{anyhow, bail, Context, Result};

use log::warn;
use regex::Regex;
use serde::Serialize;
use serde_derive::Deserialize;
use std::collections::HashMap;
use std::fs;

use std::net::IpAddr;
use std::path::{Path, PathBuf};

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

#[derive(Debug, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    global: GlobalConfig,

    #[serde(default)]
    host_defaults: HostConfig,
    #[serde(default)]
    hosts: HashMap<String, HostConfig>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct NearKeyFile {
    pub account_id: String,
    pub public_key: String,
    // Credential files generated which near cli works with have private_key
    // rather than secret_key field.  To make it possible to read those from
    // neard add private_key as an alias to this field so either will work.
    #[serde(alias = "private_key")]
    pub secret_key: String,
}

#[derive(Debug, Default, Deserialize)]
struct HostConfig {
    #[serde(default)]
    ipv4_address: Option<IpAddr>,
    #[serde(default)]
    ipv4_gateway: Option<IpAddr>,
    #[serde(default)]
    ipv4_cidr: Option<u8>,
    #[serde(default)]
    nixos_module: Option<String>,
    #[serde(default)]
    extra_nixos_modules: Vec<String>,

    #[serde(default)]
    pub mac_address: Option<String>,
    #[serde(default)]
    ipv6_address: Option<IpV6String>,
    #[serde(default)]
    ipv6_gateway: Option<IpAddr>,
    #[serde(default)]
    ipv6_cidr: Option<u8>,

    #[serde(default)]
    public_ssh_keys: Vec<String>,

    #[serde(default)]
    install_ssh_user: Option<String>,

    #[serde(default)]
    ssh_hostname: Option<String>,

    #[serde(default)]
    pub disks: Option<Vec<PathBuf>>,
}

/// Global configuration affecting all hosts
#[derive(Debug, Default, Deserialize)]
pub struct GlobalConfig {
    #[serde(default)]
    flake: Option<String>,
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
    pub ipv4_address: IpAddr,
    /// Cidr of the public ipv4 address
    pub ipv4_cidr: u8,
    /// Public ipv4 gateway ip address
    pub ipv4_gateway: IpAddr,

    /// Public ipv6 address of the host
    pub ipv6_address: Option<IpAddr>,
    /// Cidr of the public ipv6 address
    pub ipv6_cidr: Option<u8>,
    /// Public ipv6 gateway address of the host
    pub ipv6_gateway: Option<IpAddr>,

    /// SSH Username used when connecting during installation
    pub install_ssh_user: String,

    /// SSH hostname used for connecting
    pub ssh_hostname: String,

    /// Public ssh keys that will be added to the nixos configuration
    pub public_ssh_keys: Vec<String>,

    /// Block device paths to use for installing
    pub disks: Vec<PathBuf>,
}

impl Host {
    /// Returns prepared secrets directory for host
    pub fn secrets(&self) -> Result<Secrets> {
        let secret_files = vec![];
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
#[derive(Debug, PartialEq, Eq, Clone, Deserialize)]
pub struct Global {
    /// Flake url where the nixos configuration is
    #[serde(default)]
    pub flake: String,
}

fn validate_global(global_config: &GlobalConfig) -> Result<Global> {
    let default_flake = "github:kuutamolabs/lightning-knd";
    let flake = global_config
        .flake
        .as_deref()
        .unwrap_or(default_flake)
        .to_string();
    Ok(Global { flake })
}

fn validate_host(
    name: &str,
    host: &HostConfig,
    default: &HostConfig,
    _working_directory: Option<&Path>,
) -> Result<Host> {
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

    let ipv4_address = host
        .ipv4_address
        .with_context(|| format!("no ipv4_address provided for host.{name}"))?;
    let ipv4_cidr = host
        .ipv4_cidr
        .or(default.ipv4_cidr)
        .with_context(|| format!("no ipv4_cidr provided for hosts.{name}"))?;

    if !ipv4_address.is_ipv4() {
        format!("ipv4_address provided for hosts.{name} is not an ipv4 address: {ipv4_address}");
    }

    // FIXME: this is currently an unstable feature
    //if ipv4_address.is_global() {
    //    warn!("ipv4_address provided for hosts.{} is not a public ipv4 address: {}. This might not work with near mainnet", name, ipv4_address);
    //}

    if !(0..32_u8).contains(&ipv4_cidr) {
        bail!("ipv4_cidr for hosts.{name} is not between 0 and 32: {ipv4_cidr}")
    }

    let nixos_module = host
        .nixos_module
        .as_deref()
        .with_context(|| format!("no nixos_module provided for hosts.{name}"))?
        .to_string();

    let mut extra_nixos_modules = vec![];
    extra_nixos_modules.extend_from_slice(&host.extra_nixos_modules);
    extra_nixos_modules.extend_from_slice(&default.extra_nixos_modules);

    let ipv4_gateway = host
        .ipv4_gateway
        .or(default.ipv4_gateway)
        .with_context(|| format!("no ipv4_gateway provided for hosts.{name}"))?;

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
        //    warn!("ipv6_address provided for hosts.{} is not a public ipv6 address: {}. This might not work with near mainnet", name, ipv6_address);
        //}

        (Some(ipv6_address), mask)
    } else {
        warn!("No ipv6_address provided");
        (None, None)
    };

    let ssh_hostname = host
        .ssh_hostname
        .as_ref()
        .or(default.ssh_hostname.as_ref())
        .cloned()
        .unwrap_or_else(|| ipv4_address.to_string());

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
    })
}

/// Validated configuration
pub struct Config {
    /// Hosts as defined in the configuration
    pub hosts: HashMap<String, Host>,
    /// Configuration affecting all hosts
    pub global: Global,
}

/// Parse toml configuration
pub fn parse_config(content: &str, working_directory: Option<&Path>) -> Result<Config> {
    let mut config: ConfigFile = toml::from_str(content)?;
    let hosts = config
        .hosts
        .iter_mut()
        .map(|(name, host)| {
            Ok((
                name.clone(),
                validate_host(name, host, &config.host_defaults, working_directory)?,
            ))
        })
        .collect::<Result<_>>()?;

    let global = validate_global(&config.global)?;
    Ok(Config { hosts, global })
}

/// Load configuration from path
pub fn load_configuration(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path).context("Cannot read file")?;
    let working_directory = path.parent();
    parse_config(&content, working_directory)
}

#[test]
pub fn test_parse_config() -> Result<()> {
    use std::str::FromStr;

    let config_str = r#"
[global]
flake = "github:myfork/near-staking-knd"

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
nixos_module = "kld-node"
ipv4_address = "199.127.64.3"
ipv6_address = "2605:9880:400::3"

[hosts.db-01]
nixos_module = "cockroachdb-node"
ipv4_address = "199.127.64.4"
ipv6_address = "2605:9880:400::4"
"#;

    let config = parse_config(config_str, None)?;
    assert_eq!(config.global.flake, "github:myfork/near-staking-knd");

    let hosts = &config.hosts;
    assert_eq!(hosts.len(), 3);
    assert_eq!(
        hosts["kld-00"].ipv4_address,
        IpAddr::from_str("199.127.64.2").unwrap()
    );
    assert_eq!(hosts["kld-00"].ipv4_cidr, 24);
    assert_eq!(
        hosts["db-00"].ipv4_gateway,
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

    parse_config(config_str, None)?;

    Ok(())
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
fn test_validate_host() {
    let mut config = HostConfig {
        ipv4_address: Some("192.168.0.1".parse::<IpAddr>().unwrap()),
        nixos_module: Some("kld-node".to_string()),
        ipv4_cidr: Some(0),
        ipv4_gateway: Some("192.168.255.255".parse::<IpAddr>().unwrap()),
        ipv6_address: None,
        ipv6_gateway: None,
        ipv6_cidr: None,
        public_ssh_keys: vec!["".to_string()],
        ..Default::default()
    };
    assert_eq!(
        validate_host("ipv4-only", &config, &HostConfig::default(), None).unwrap(),
        Host {
            name: "ipv4-only".to_string(),
            nixos_module: "kld-node".to_string(),
            extra_nixos_modules: Vec::new(),
            mac_address: None,
            ipv4_address: "192.168.0.1".parse::<IpAddr>().unwrap(),
            ipv4_cidr: 0,
            ipv4_gateway: "192.168.255.255".parse::<IpAddr>().unwrap(),
            ipv6_address: None,
            ipv6_cidr: None,
            ipv6_gateway: None,
            install_ssh_user: "root".to_string(),
            ssh_hostname: "192.168.0.1".to_string(),
            public_ssh_keys: vec!["".to_string()],
            disks: vec!["/dev/nvme0n1".into(), "/dev/nvme1n1".into()],
        }
    );

    // If `ipv6_address` is provied, the `ipv6_gateway` and `ipv6_cidr` should be provided too,
    // else the error will raise
    config.ipv6_address = Some("2607:5300:203:6cdf::".into());
    assert!(validate_host("ipv4-only", &config, &HostConfig::default(), None).is_err());

    config.ipv6_gateway = Some(
        "2607:5300:0203:6cff:00ff:00ff:00ff:00ff"
            .parse::<IpAddr>()
            .unwrap(),
    );
    assert!(validate_host("ipv4-only", &config, &HostConfig::default(), None).is_err());

    // The `ipv6_cidr` could be provided by subnet in address field
    config.ipv6_address = Some("2607:5300:203:6cdf::/64".into());
    assert!(validate_host("ipv4-only", &config, &HostConfig::default(), None).is_ok());
}
