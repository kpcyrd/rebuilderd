use crate::api::header;
use crate::config::Config;
use crate::models::NewWorker;
use crate::{models, web};
use actix_web::HttpRequest;
use chrono::Utc;
use diesel::SqliteConnection;
use log::debug;
use rebuilderd_common::api::WORKER_KEY_HEADER;
use rebuilderd_common::errors::{format_err, Context};
use std::net::IpAddr;

pub fn refresh_worker(
    req: &HttpRequest,
    cfg: &Config,
    connection: &mut SqliteConnection,
) -> web::Result<models::Worker> {
    let key = header(req, WORKER_KEY_HEADER).context("Failed to get worker key")?;

    let ip = if let Some(real_ip_header) = &cfg.real_ip_header {
        let ip = header(req, real_ip_header).context("Failed to locate real ip header")?;
        ip.parse::<IpAddr>()
            .context("Can't parse real ip header as ip address")?
    } else {
        let ci = req
            .peer_addr()
            .ok_or_else(|| format_err!("Can't determine client ip"))?;
        ci.ip()
    };

    debug!("detected worker ip for {:?} as {}", key, ip);

    let new_worker = NewWorker {
        key: key.to_string(),
        name: "".to_string(), // TODO: name handling
        address: ip.to_string(),
        status: None,
        last_ping: Utc::now().naive_utc(),
        online: true,
    };

    // TODO: this overwrites status and name
    Ok(new_worker.upsert(connection)?)
}
