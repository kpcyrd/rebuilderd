use crate::auth;
use rebuilderd_common::config::{ConfigFile, ScheduleConfig, WorkerConfig};
use rebuilderd_common::errors::*;
use std::env;
use std::fs;
use std::path::Path;

const DEFAULT_POST_BODY_SIZE_LIMIT: usize = 2_usize.pow(30); // 1 GB

#[derive(Debug, Clone)]
pub struct Config {
    pub auth_cookie: String,
    pub worker: WorkerConfig,
    pub bind_addr: String,
    pub real_ip_header: Option<String>,
    pub post_body_size_limit: usize,
    pub schedule: ScheduleConfig,
}

pub fn from_struct(config: ConfigFile, auth_cookie: String) -> Result<Config> {
    let bind_addr = if let Ok(addr) = env::var("HTTP_ADDR") {
        addr
    } else if let Some(addr) = config.http.bind_addr {
        addr
    } else {
        "127.0.0.1:8484".to_string()
    };

    Ok(Config {
        auth_cookie,
        worker: config.worker,
        bind_addr,
        real_ip_header: config.http.real_ip_header,
        post_body_size_limit: config
            .http
            .post_body_size_limit
            .unwrap_or(DEFAULT_POST_BODY_SIZE_LIMIT),
        schedule: config.schedule,
    })
}

pub fn load(path: Option<&Path>) -> Result<Config> {
    let config = if let Some(path) = path {
        let buf = fs::read_to_string(path).context("Failed to read config file")?;
        toml::from_str(&buf)?
    } else {
        ConfigFile::default()
    };

    let auth_cookie = auth::setup_auth_cookie().context("Failed to setup auth cookie")?;

    from_struct(config, auth_cookie)
}
