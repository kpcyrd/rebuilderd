use crate::errors::*;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::Path;

const SYSTEM_CONFIG_PATH: &str = "/etc/rebuilderd.conf";
const SYSTEM_COOKIE_PATH: &str = "/var/lib/rebuilderd/auth-cookie";

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub auth: AuthConfig,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct AuthConfig {
    pub cookie: Option<String>,
}

impl AuthConfig {
    pub fn update(&mut self, c: AuthConfig) {
        if c.cookie.is_some() {
            self.cookie = c.cookie;
        }
    }
}

fn read_cookie_from_config<P: AsRef<Path>>(path: P) -> Result<Option<String>> {
    debug!("Attempting reading cookie from config: {:?}", path.as_ref());
    if let Ok(buf) = fs::read_to_string(path.as_ref()) {
        let config = toml::from_str::<Config>(&buf)?;
        debug!("Found cookie in config {:?}", path.as_ref());
        Ok(config.auth.cookie)
    } else {
        Ok(None)
    }
}

pub fn read_cookie_from_file<P: AsRef<Path>>(path: P) -> Result<String> {
    debug!("Attempting reading cookie from file: {:?}", path.as_ref());
    let cookie = fs::read_to_string(path.as_ref())?;
    debug!("Found cookie in file {:?}", path.as_ref());
    Ok(cookie.trim().to_string())
}

pub fn find_auth_cookie() -> Result<String> {
    if let Ok(cookie_path) = env::var("REBUILDERD_COOKIE_PATH") {
        debug!("Using auth cookie from cookie path env: {cookie_path:?}");
        return read_cookie_from_file(cookie_path);
    }

    if let Some(config_dir) = dirs_next::config_dir() {
        let path = config_dir.join("rebuilderd.conf");
        if let Some(cookie) = read_cookie_from_config(&path)? {
            debug!("Using auth cookie from user-config: {path:?}");
            return Ok(cookie);
        }
    }

    if let Some(cookie) = read_cookie_from_config(SYSTEM_CONFIG_PATH)? {
        debug!("Using auth cookie from system-config: {SYSTEM_CONFIG_PATH:?}");
        return Ok(cookie);
    }

    if let Ok(cookie) = read_cookie_from_file(SYSTEM_COOKIE_PATH) {
        debug!("Using auth cookie from system-daemon: {SYSTEM_COOKIE_PATH:?}");
        return Ok(cookie);
    }

    if let Some(data_dir) = dirs_next::data_dir() {
        let path = data_dir.join("rebuilderd-auth-cookie");
        if let Ok(cookie) = read_cookie_from_file(&path) {
            debug!("Using auth cookie from user-daemon: {path:?}");
            return Ok(cookie);
        }
    }

    bail!("Failed to find auth cookie anywhere")
}
