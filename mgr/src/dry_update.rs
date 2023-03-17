use anyhow::Result;
use log::info;
use std::path::Path;

use crate::nixos_rebuild;

use super::{Host, NixosFlake};

/// Push update to server but do not activate it yet.
pub fn dry_update(hosts: &[Host], flake: &NixosFlake, secrets_dir: &Path) -> Result<()> {
    flake.show()?;
    hosts
        .iter()
        .map(|host| {
            info!("Dry-update {}", host.name);

            nixos_rebuild("dry-activate", host, flake, secrets_dir, false)?;

            Ok(())
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(())
}
