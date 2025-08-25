use rand::distr::{Alphanumeric, SampleString};
use rebuilderd_common::auth;
use rebuilderd_common::config::{ConfigFile, ScheduleConfig, WorkerConfig};
use rebuilderd_common::errors::*;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

const DEFAULT_POST_BODY_SIZE_LIMIT: usize = 2_usize.pow(30); // 1 GB

#[derive(Debug, Clone)]
pub struct Config {
    pub auth_cookie: String,
    pub worker: WorkerConfig,
    pub bind_addr: String,
    pub real_ip_header: Option<String>,
    pub post_body_size_limit: usize,
    pub transparently_sign_attestations: bool,
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
        transparently_sign_attestations: config
            .http
            .transparently_sign_attestations
            .unwrap_or(true),
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

    let auth_cookie = setup_auth_cookie().context("Failed to setup auth cookie")?;

    from_struct(config, auth_cookie)
}

pub fn setup_auth_cookie() -> Result<String> {
    let cookie = if let Ok(cookie) = auth::find_auth_cookie() {
        debug!("Loaded cookie from filesystem");
        cookie
    } else {
        debug!("Generating random cookie");
        Alphanumeric.sample_string(&mut rand::rng(), 32)
    };

    let cookie_path = if let Ok(cookie_path) = env::var("REBUILDERD_COOKIE_PATH") {
        PathBuf::from(cookie_path)
    } else if let Some(data_dir) = dirs_next::data_dir() {
        data_dir.join("rebuilderd-auth-cookie")
    } else {
        PathBuf::from("./auth-cookie")
    };

    if let Some(parent) = cookie_path.parent() {
        debug!(
            "Ensuring parent directory for auth cookie exists: {:?}",
            parent
        );
        fs::create_dir_all(parent)?;
    }

    debug!("Writing auth cookie to {:?}", cookie_path);
    let mut file = OpenOptions::new()
        .mode(0o640)
        .write(true)
        .create(true)
        .open(cookie_path)
        .context("Failed to open auth cookie file")?;
    file.write_all(format!("{}\n", cookie).as_bytes())?;

    Ok(cookie)
}
