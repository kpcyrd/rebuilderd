use rebuilderd_common::errors::*;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Default, Deserialize)]
pub struct ConfigFile {
    pub endpoint: Option<String>,
    pub signup_secret: Option<String>,
    // this option is deprecated, use diffoscope.enabled instead
    #[serde(default)]
    pub gen_diffoscope: bool,
    #[serde(default)]
    pub build: Build,
    #[serde(default)]
    pub diffoscope: Diffoscope,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Build {
    pub timeout: Option<u64>,
    pub max_bytes: Option<usize>,
    #[serde(default)]
    pub silent: bool,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Diffoscope {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub args: Vec<String>,
    pub timeout: Option<u64>,
    pub max_bytes: Option<usize>,
}

pub fn load(path: Option<&Path>) -> Result<ConfigFile> {
    let path = path.unwrap_or_else(|| Path::new("/etc/rebuilderd-worker.conf"));
    if path.exists() {
        let buf = fs::read(path)
            .with_context(|| anyhow!("Failed to open {:?}", path))?;
        let mut conf = toml::from_slice::<ConfigFile>(&buf)?;

        if conf.gen_diffoscope {
            warn!("Option gen_diffoscope is deprecated, use diffoscope.enabled instead");
            conf.diffoscope.enabled = true;
        }

        Ok(conf)
    } else {
        Ok(ConfigFile::default())
    }
}
