use rebuilderd_common::errors::*;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Default, Deserialize)]
pub struct ConfigFile {
    pub endpoint: Option<String>,
}

pub fn load() -> Result<ConfigFile> {
    let path = Path::new("/etc/rebuilderd-worker.conf");
    if path.exists() {
        let buf = fs::read(path)?;
        let conf = toml::from_slice(&buf)?;
        Ok(conf)
    } else {
        Ok(ConfigFile::default())
    }
}
