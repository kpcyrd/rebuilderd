use crate::auth::AuthConfig;
use serde::Deserialize;

pub const IDLE_DELAY: u64 = 180;
pub const PING_DEADLINE: i64 = IDLE_DELAY as i64 + 20;
pub const PING_INTERVAL: u64 = 30;
pub const WORKER_DELAY: u64 = 3;
pub const API_ERROR_DELAY: u64 = 30;

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
    pub endpoint: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct WorkerConfig {
    #[serde(default)]
    pub authorized_workers: Vec<String>,
    pub signup_secret: Option<String>,
}
