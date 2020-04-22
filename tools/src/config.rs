use rebuilderd_common::Distro;
use rebuilderd_common::config::ConfigFile;
use rebuilderd_common::errors::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct SyncConfigFile {
    #[serde(rename="profile")]
    pub profiles: HashMap<String, SyncProfile>,
}

impl SyncConfigFile {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<SyncConfigFile> {
        let buf = fs::read(path)
            .context("Failed to read config file")?;
        let config = toml::from_slice(&buf)
            .context("Failed to load config")?;
        Ok(config)
    }
}

#[derive(Debug, Deserialize)]
pub struct SyncProfile {
    #[serde(default)]
    pub maintainers: Vec<String>,
    #[serde(default)]
    pub pkgs: Vec<String>,
    pub distro: Distro,
    pub suite: String,
    pub architecture: String,
    pub source: String,
}

pub fn load<P: AsRef<Path>>(path: Option<P>) -> Result<ConfigFile> {
    if let Some(path) = path {
        load_from(path)
    } else {
        let path = config_path()?;
        load_from(path)
    }
}

pub fn config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| format_err!("Failed to find config dir"))?;
    Ok(config_dir.join("rebuilderd.conf"))
}

pub fn load_from<P: AsRef<Path>>(path: P) -> Result<ConfigFile> {
    if let Ok(buf) = fs::read(path) {
        let config = toml::from_slice(&buf)
            .context("Failed to load config")?;
        Ok(config)
    } else {
        Ok(ConfigFile::default())
    }
}
