use crate::auth;
use rebuilderd_common::auth::AuthConfig;
use rebuilderd_common::errors::*;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Config {
    pub auth_cookie: String,
    pub worker: WorkerConfig,
    pub bind_addr: String,
}

pub fn load(path: Option<&Path>) -> Result<Config> {
    let config = if let Some(path) = path {
        let buf = fs::read(path)
            .context("Failed to read config file")?;
        toml::from_slice(&buf)?
    } else {
        ConfigFile::default()
    };

    let auth_cookie = auth::setup_auth_cookie()
        .context("Failed to setup auth cookie")?;

    let bind_addr = if let Ok(addr) = env::var("HTTP_ADDR") {
        addr
    } else if let Some(addr) = config.http.bind_addr {
        addr
    } else {
        "127.0.0.1:8080".to_string()
    };

    Ok(Config {
        auth_cookie,
        worker: config.worker,
        bind_addr,
    })
}

#[derive(Debug, Default, Deserialize)]
pub struct ConfigFile {
    #[serde(default)]
    pub http: HttpConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub worker: WorkerConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct HttpConfig {
    pub bind_addr: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct WorkerConfig {
    #[serde(default)]
    pub authorized_workers: Vec<String>,
    pub signup_secret: Option<String>,
}
