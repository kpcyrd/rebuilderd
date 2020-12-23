use rebuilderd_common::Distro;
use rebuilderd_common::errors::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

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
    pub distro: Distro,
    pub suite: String,
    #[serde(default)]
    pub releases: Vec<String>,
    pub architecture: String,
    pub source: String,

    #[serde(default)]
    pub maintainers: Vec<String>,
    #[serde(default)]
    pub pkgs: Vec<String>,
    #[serde(default)]
    pub excludes: Vec<String>,
}
