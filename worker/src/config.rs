use crate::args::Args;
use rebuilderd_common::errors::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ConfigFile {
    pub endpoint: Option<String>,
    pub signup_secret: Option<String>,
    #[serde(default)]
    pub build: Build,
    #[serde(default)]
    pub diffoscope: Diffoscope,
    #[serde(default, rename = "backend")]
    pub backends: HashMap<String, Backend>,
    #[serde(default)]
    pub supported_architectures: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Build {
    pub timeout: Option<u64>,
    pub max_bytes: Option<usize>,
    #[serde(default)]
    pub silent: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Diffoscope {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub args: Vec<String>,
    pub timeout: Option<u64>,
    pub max_bytes: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Backend {
    pub path: PathBuf,
}

pub fn load(args: &Args) -> Result<ConfigFile> {
    let path = if let Some(path) = args.config.as_ref() {
        Some(path.to_owned())
    } else {
        let path = PathBuf::from("/etc/rebuilderd-worker.conf");
        if path.exists() {
            warn!("Using the implicit `-c /etc/rebuilderd-worker.conf` is going to be removed in the future");
            Some(path)
        } else {
            None
        }
    };

    let mut conf = if let Some(path) = path {
        info!("Loading configuration from {:?}", path);
        let buf =
            fs::read_to_string(&path).with_context(|| anyhow!("Failed to open {:?}", path))?;
        toml::from_str::<ConfigFile>(&buf)?
    } else {
        info!("Using default configuration");
        ConfigFile::default()
    };

    for backend in &args.backends {
        debug!("Adding to list of supported backends: {:?}", backend);
        let (key, path) = backend.split_once('=').ok_or_else(|| {
            anyhow!("Invalid argument, expected format is --backend distro=/path/to/script")
        })?;

        conf.backends
            .insert(key.into(), Backend { path: path.into() });
    }

    Ok(conf)
}
