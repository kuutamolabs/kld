use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

use super::NixosFlake;

// only copies files no directories
fn copy_dir_all<A: AsRef<Path>>(src: impl AsRef<Path>, dst: A) -> Result<()> {
    fs::create_dir_all(&dst).with_context(|| {
        format!(
            "Cannot creating target directory '{}'",
            dst.as_ref().display()
        )
    })?;
    for entry in fs::read_dir(&src)? {
        let entry = entry.with_context(|| {
            format!(
                "Cannot list directory content for '{}'",
                src.as_ref().display()
            )
        })?;
        fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
    }
    Ok(())
}

/// Install a Validator on a given machine
pub fn generate_config(directory: &PathBuf, flake: &NixosFlake) -> Result<()> {
    copy_dir_all(flake.path(), directory).context("failed to copy flake")
}
