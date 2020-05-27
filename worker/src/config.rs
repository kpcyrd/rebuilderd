use rebuilderd_common::errors::*;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Default, Deserialize)]
pub struct ConfigFile {
    pub endpoint: Option<String>,
    pub signup_secret: Option<String>,
    #[serde(default)]
    pub gen_diffoscope: bool,
}

pub fn load(path: Option<&Path>) -> Result<ConfigFile> {
    let path = path.unwrap_or_else(|| Path::new("/etc/rebuilderd-worker.conf"));
    if path.exists() {
        let buf = fs::read(path)?;
        let conf = toml::from_slice(&buf)?;
        Ok(conf)
    } else {
        Ok(ConfigFile::default())
    }
}
