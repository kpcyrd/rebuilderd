use crate::api;
use crate::config::Config;
use crate::models::Worker;
use crate::schema::workers;
use actix_web::HttpRequest;
use diesel::QueryDsl;
use diesel::SqliteExpressionMethods;
use diesel::{RunQueryDsl, SqliteConnection};
use log::debug;
use rebuilderd_common::api::{AUTH_COOKIE_HEADER, SIGNUP_SECRET_HEADER, WORKER_KEY_HEADER};
use rebuilderd_common::errors::{bail, Context};

pub fn admin(cfg: &Config, req: &HttpRequest) -> rebuilderd_common::errors::Result<()> {
    let auth_cookie = api::header(req, AUTH_COOKIE_HEADER).context("Failed to get auth cookie")?;

    if cfg.auth_cookie != auth_cookie {
        bail!("Wrong auth cookie")
    }

    Ok(())
}

pub fn worker(
    cfg: &Config,
    req: &HttpRequest,
    connection: &mut SqliteConnection,
) -> rebuilderd_common::errors::Result<Worker> {
    // Check if auth is required BEFORE trying to extract the header
    if cfg.worker.authorized_workers.is_empty() && cfg.worker.signup_secret.is_none() {
        // When no auth is configured, try to get the worker key if provided, otherwise use a default key
        let worker_key = api::header(req, WORKER_KEY_HEADER).unwrap_or("unauthenticated");
        return Worker::get_or_create(worker_key, "anonymous", connection);
    }

    // Auth is configured, so the worker key is required
    let worker_key = api::header(req, WORKER_KEY_HEADER).context("Failed to get worker key")?;

    if !cfg.worker.authorized_workers.is_empty()
        && !cfg
            .worker
            .authorized_workers
            .iter()
            .any(|x| x == worker_key)
    {
        bail!("Worker key is not on allow-list");
    }

    let key_is_registered = diesel::dsl::select(diesel::dsl::exists(
        workers::table.filter(workers::key.is(worker_key)),
    ))
    .get_result::<bool>(connection)?;

    if !key_is_registered {
        bail!("Worker is not registered")
    }

    let worker = Worker::get_and_refresh(worker_key, connection)?;
    Ok(worker)
}

pub fn signup(cfg: &Config, req: &HttpRequest) -> rebuilderd_common::errors::Result<()> {
    let worker_key = api::header(req, WORKER_KEY_HEADER).context("Failed to get worker key")?;

    if !cfg.worker.authorized_workers.is_empty()
        && !cfg
            .worker
            .authorized_workers
            .iter()
            .any(|x| x == worker_key)
    {
        bail!("Worker key is not on allow-list");
    }

    if let Some(expected_signup_secret) = &cfg.worker.signup_secret {
        let signup_secret =
            api::header(req, SIGNUP_SECRET_HEADER).context("Failed to get signup secret")?;

        if signup_secret == expected_signup_secret {
            debug!("worker authenticated with signup secret");
            return Ok(());
        } else {
            bail!("Signup secret mismatched");
        }
    }

    Ok(())
}
