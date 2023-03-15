use anyhow::Result;
use log::info;

use crate::nixos_rebuild;

use super::{Host, NixosFlake};

/// Rollback a nixos machine
pub fn rollback(hosts: &[Host], flake: &NixosFlake) -> Result<()> {
    flake.show()?;
    hosts
        .iter()
        .map(|host| {
            info!("Rollback {}", host.name);

            nixos_rebuild("rollback", host, flake, false)?;

            Ok(())
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(())
}
