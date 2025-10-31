use crate::api;
use crate::config::Config;
use actix_web::HttpRequest;
use rebuilderd_common::api::AUTH_COOKIE_HEADER;
use rebuilderd_common::errors::*;

pub fn admin(cfg: &Config, req: &HttpRequest) -> Result<()> {
    let auth_cookie = api::header(req, AUTH_COOKIE_HEADER).context("Failed to get auth cookie")?;

    if cfg.auth_cookie != auth_cookie {
        bail!("Wrong auth cookie")
    }

    Ok(())
}
