use anyhow::{Context, Result};
use std::io::Write;
use std::process::Command;
use std::{fs::File, path::Path};
use tempfile::{Builder, TempDir};

use super::command::status_to_pretty_err;
use super::Config;

/// The nixos flake
pub struct NixosFlake {
    tmp_dir: TempDir,
}

impl NixosFlake {
    /// Path to the nixos flake
    pub fn path(&self) -> &Path {
        self.tmp_dir.path()
    }

    /// This initializes the flake i.e. downloads all inputs but in a less
    /// verbose way than other `nix flake` commands that will print all inputs
    /// changed.
    pub fn show(&self) -> Result<()> {
        let args = vec![
            "flake",
            "show",
            "--extra-experimental-features",
            "flakes nix-command",
            self.path()
                .to_str()
                .context("failed to convert temporary directory path to string")?,
        ];
        let status = Command::new("nix").args(&args).status();
        status_to_pretty_err(status, "nix", &args).context("cannot show flake")
    }
}

/// Creates a flake directory
pub fn generate_nixos_flake(config: &Config) -> Result<NixosFlake> {
    let tmp_dir = Builder::new()
        .prefix("kuutamo-flake.")
        .tempdir()
        .context("cannot create temporary directory")?;

    let knd_flake = &config.global.knd_flake;
    for (name, host) in &config.hosts {
        let host_path = tmp_dir.path().join(format!("{name}.toml"));
        let mut host_file = File::create(&host_path)
            .with_context(|| format!("could not create {}", host_path.display()))?;
        let mut host_toml =
            toml::to_string(&host).with_context(|| format!("cannot serialize {name} to toml"))?;
        host_toml = format!(
            r#"deployment_flake = "{}"
deploy_pubkey = "{}"
"#,
            &config.global.deployment_flake,
            &config
                .global
                .deploy_pubkey
                .as_ref()
                .unwrap_or(&"This will generate when instaling".to_string())
        ) + &host_toml;
        host_file
            .write_all(host_toml.as_bytes())
            .with_context(|| format!("Cannot write {}", host_path.display()))?;
    }
    let configurations = config
        .hosts
        .iter()
        .map(|(name, host)| {
            let mut nixos_modules = vec![];
            nixos_modules.push(host.nixos_module.clone());
            nixos_modules.extend_from_slice(host.extra_nixos_modules.as_slice());

            let modules = nixos_modules
                .iter()
                .map(|m| format!("      lightning-knd.nixosModules.\"{m}\""))
                .collect::<Vec<_>>()
                .join("\n");

            format!(
                r#"  nixosConfigurations."{name}" = lightning-knd.inputs.nixpkgs.lib.nixosSystem {{
    system = "x86_64-linux";
    modules = [
{modules}
      {{ kuutamo.deployConfig = builtins.fromTOML (builtins.readFile (builtins.path {{ name = "node.toml"; path = ./{name}.toml; }})); }}
    ];
  }};"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let configuration_path = tmp_dir.path().join("configurations.nix");
    let mut configuration_file =
        File::create(configuration_path).context("could not create configurations.nix")?;
    let configuration_content = format!(
        r#"{{ lightning-knd, ... }}: {{
{configurations}
}}
"#
    );
    configuration_file
        .write_all(configuration_content.as_bytes())
        .context("could not write configurations.nix")?;
    let flake_content = format!(
        r#"{{
  inputs.lightning-knd.url = "{knd_flake}";

  nixConfig.extra-substituters = [
    "https://cache.garnix.io"
  ];

  nixConfig.extra-trusted-public-keys = [
    "cache.garnix.io:CTFPyKSLcx5RMJKfLo5EEPUObbA78b0YQ2DTCJXqr9g="
  ];

  outputs = inputs: import ./configurations.nix inputs;
}}
"#
    );
    let flake_path = tmp_dir.path().join("flake.nix");
    let mut flake_file = File::create(flake_path).context("could not create flake.nix")?;
    flake_file
        .write_all(flake_content.as_bytes())
        .context("could not write flake.nix")?;
    Ok(NixosFlake { tmp_dir })
}

#[test]
pub fn test_nixos_flake() -> Result<()> {
    use crate::config::{parse_config, TEST_CONFIG};
    use std::process::Command;

    let config = parse_config(TEST_CONFIG, Path::new("/"), false)?;
    let flake = generate_nixos_flake(&config)?;
    let flake_path = flake.path();
    let flake_nix = flake_path.join("flake.nix");
    let tmp_dir = TempDir::new()?;
    let args = vec![
        "--parse",
        flake_nix.to_str().unwrap(),
        "--store",
        tmp_dir.path().to_str().unwrap(),
    ];
    let status = Command::new("nix-instantiate").args(args).status()?;
    assert_eq!(status.code(), Some(0));
    assert!(flake_path.join("kld-00.toml").exists());
    assert!(flake_path.join("db-00.toml").exists());
    assert!(flake_path.join("db-01.toml").exists());
    Ok(())
}
