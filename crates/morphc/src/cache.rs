use anyhow::{Context, Result};
use std::path::PathBuf;

pub(crate) fn global_cache_root() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not find home directory")?;
    Ok(home.join(".morph"))
}

#[allow(dead_code)]
pub(crate) fn global_runtimes_dir() -> Result<PathBuf> {
    Ok(global_cache_root()?.join("cache").join("runtimes"))
}

#[allow(dead_code)]
pub(crate) fn global_runtime_version_dir(runtime_type: &str, version: &str) -> Result<PathBuf> {
    Ok(global_runtimes_dir()?.join(runtime_type).join(format!("v{version}")))
}
