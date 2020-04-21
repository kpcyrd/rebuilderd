// use rebuilderd_common::errors::*;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct Config {
    pub auth_cookie: String,
    pub authorized_workers: Vec<String>,
    pub signup_secret: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigFile {
}
