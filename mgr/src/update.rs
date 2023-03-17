use anyhow::Result;
use log::info;
use std::path::Path;

use crate::nixos_rebuild;

use super::{Host, NixosFlake};

/// Update Validator on a given machine
pub fn update(hosts: &[Host], flake: &NixosFlake, secrets_dir: &Path) -> Result<()> {
    flake.show()?;
    hosts
        .iter()
        .map(|host| {
            info!("Update {}", host.name);

            nixos_rebuild("switch", host, flake, secrets_dir, true)?;

            Ok(())
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(())
}
