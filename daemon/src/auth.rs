use actix_web::HttpRequest;
use crate::api;
use crate::config::Config;
use rand::prelude::*;
use rand::distributions::Alphanumeric;
use rebuilderd_common::api::*;
use rebuilderd_common::errors::*;
use std::env;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

pub fn admin(cfg: &Config, req: &HttpRequest) -> Result<()> {
    let auth_cookie = api::header(req, AUTH_COOKIE_HEADER)
        .context("Failed to get auth cookie")?;

    if cfg.auth_cookie != auth_cookie {
        bail!("Wrong auth cookie")
    }

    Ok(())
}

pub fn worker(cfg: &Config, req: &HttpRequest) -> Result<()> {
    let worker_key = api::header(req, WORKER_KEY_HEADER)
        .context("Failed to get worker key")?;

    // TODO: we do not challenge the worker keys yet
    if !cfg.authorized_workers.is_empty() {
        // Vec<String>::contains() is inefficient with &str
        if !cfg.authorized_workers.iter().any(|x| x == worker_key) {
            bail!("Worker key is not authorized")
        }
    }

    if let Some(expected_signup_secret) = &cfg.signup_secret {
        let signup_secret = api::header(req, SIGNUP_SECRET_HEADER)
            .context("Failed to get worker key")?;

        if signup_secret != expected_signup_secret {
            bail!("Incorrect signup secret")
        }
    }

    Ok(())
}

pub fn setup_auth_cookie() -> Result<String> {
    let cookie = if let Ok(cookie) = rebuilderd_common::auth::find_auth_cookie() {
        debug!("Loaded cookie from filesystem");
        cookie
    } else {
        debug!("Generating random cookie");
        thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .collect()
    };

    let cookie_path = if let Ok(cookie_path) = env::var("REBUILDERD_COOKIE_PATH") {
        PathBuf::from(cookie_path)
    } else if let Some(data_dir) = dirs::data_dir() {
        data_dir.join("rebuilderd-auth-cookie")
    } else {
        PathBuf::from("auth-cookie")
    };

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
