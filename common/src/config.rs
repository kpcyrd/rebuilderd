use crate::auth::AuthConfig;
use crate::errors::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const IDLE_DELAY: u64 = 180;
pub const PING_DEADLINE: i64 = IDLE_DELAY as i64 + 20;
pub const PING_INTERVAL: u64 = 60;
pub const WORKER_DELAY: u64 = 3;
pub const API_ERROR_DELAY: u64 = 30;

pub const DEFAULT_RETRY_DELAY_BASE: i64 = 24;

pub fn load<P: AsRef<Path>>(path: Option<P>) -> Result<ConfigFile> {
    let mut config = ConfigFile::default();

    if let Some(c) = load_from("/etc/rebuilderd.conf")? {
        config.update(c);
    }

    if let Ok(path) = config_path() {
        if let Some(c) = load_from(path)? {
            config.update(c);
        }
    }

    if let Some(path) = path {
        let c = load_from(path)?
            .ok_or_else(|| format_err!("Failed to read config file"))?;
        config.update(c);
    }

    Ok(config)
}

fn config_path() -> Result<PathBuf> {
    let config_dir = dirs_next::config_dir()
        .ok_or_else(|| format_err!("Failed to find config dir"))?;
    Ok(config_dir.join("rebuilderd.conf"))
}

fn load_from<P: AsRef<Path>>(path: P) -> Result<Option<ConfigFile>> {
    if let Ok(buf) = fs::read_to_string(path.as_ref()) {
        debug!("loading config file {:?}", path.as_ref());
        let config = toml::from_str(&buf)
            .context("Failed to load config")?;
        Ok(Some(config))
    } else {
        Ok(None)
    }
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct ConfigFile {
    #[serde(default)]
    pub http: HttpConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub endpoints: HashMap<String, EndpointConfig>,
    #[serde(default)]
    pub worker: WorkerConfig,
    #[serde(default)]
    pub schedule: ScheduleConfig,
}

impl ConfigFile {
    pub fn update(&mut self, c: ConfigFile) {
        self.http.update(c.http);
        self.auth.update(c.auth);
        for (k, v) in c.endpoints {
            if let Some(o) = self.endpoints.get_mut(&k) {
                o.update(v);
            } else {
                self.endpoints.insert(k, v);
            }
        }
        self.worker.update(c.worker);
        self.schedule.update(c.schedule);
    }
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct HttpConfig {
    pub bind_addr: Option<String>,
    pub real_ip_header: Option<String>,
    pub post_body_size_limit: Option<usize>,
    pub endpoint: Option<String>,
}

impl HttpConfig {
    pub fn update(&mut self, c: HttpConfig) {
        if c.bind_addr.is_some() {
            self.bind_addr = c.bind_addr;
        }
        if c.real_ip_header.is_some() {
            self.real_ip_header = c.real_ip_header;
        }
        if c.endpoint.is_some() {
            self.endpoint = c.endpoint;
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct EndpointConfig {
    pub cookie: String,
}

impl EndpointConfig {
    pub fn update(&mut self, c: EndpointConfig) {
        self.cookie = c.cookie;
    }
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct WorkerConfig {
    #[serde(default)]
    pub authorized_workers: Vec<String>,
    pub signup_secret: Option<String>,
}

impl WorkerConfig {
    pub fn update(&mut self, c: WorkerConfig) {
        if !c.authorized_workers.is_empty() {
            self.authorized_workers = c.authorized_workers;
        }
        if c.signup_secret.is_some() {
            self.signup_secret = c.signup_secret;
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct ScheduleConfig {
    retry_delay_base: Option<i64>,
}

impl ScheduleConfig {
    pub fn update(&mut self, c: ScheduleConfig) {
        if c.retry_delay_base.is_some() {
            self.retry_delay_base = c.retry_delay_base;
        }
    }

    pub fn retry_delay_base(&self) -> i64 {
        self.retry_delay_base.unwrap_or(DEFAULT_RETRY_DELAY_BASE)
    }
}
