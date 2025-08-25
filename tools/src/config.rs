use rebuilderd_common::errors::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct SyncConfigFile {
    #[serde(rename = "profile")]
    pub profiles: HashMap<String, SyncProfile>,
}

impl SyncConfigFile {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<SyncConfigFile> {
        let buf = fs::read_to_string(path).context("Failed to read config file")?;
        let config = toml::from_str(&buf).context("Failed to load config")?;
        Ok(config)
    }
}

#[derive(Debug, Deserialize)]
pub struct SyncProfile {
    pub distro: String,
    pub sync_method: Option<String>,
    pub suite: Option<String>,
    #[serde(default)]
    pub releases: Vec<String>,
    pub architecture: Option<String>,
    #[serde(default)]
    pub architectures: Vec<String>,
    pub source: String,

    #[serde(default)]
    pub maintainers: Vec<String>,
    #[serde(default)]
    pub pkgs: Vec<String>,
    #[serde(default)]
    pub excludes: Vec<String>,
}
