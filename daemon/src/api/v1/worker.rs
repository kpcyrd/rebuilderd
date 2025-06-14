use crate::api::v1::util::pagination::{Page, PaginateDsl};
use crate::diesel::ExpressionMethods;
use crate::{auth, models, web};
use std::net::IpAddr;

use crate::api::v0::header;
use crate::config::Config;
use crate::db::Pool;
use crate::schema::{source_packages, workers};
use actix_web::{delete, get, post, HttpRequest, HttpResponse, Responder};
use diesel::{Connection, OptionalExtension, QueryDsl, RunQueryDsl, SqliteConnection};
use log::debug;
use rebuilderd_common::api::v0::WORKER_KEY_HEADER;
use rebuilderd_common::api::v1::{RegisterWorkerRequest, ResultPage};
use rebuilderd_common::errors::{format_err, Context, Error};

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

    if let Some(mut worker) = models::Worker::get(key, connection)? {
        worker.bump_last_ping(&ip);
        Ok(worker)
    } else {
        let worker = models::NewWorker::new(key.to_string(), ip, None);
        worker.insert(connection)?;
        refresh_worker(req, cfg, connection)
    }
}

#[diesel::dsl::auto_type]
fn workers_base() -> _ {
    workers::table.select((
        workers::id,
        workers::name,
        workers::address,
        workers::status,
        workers::last_ping,
        workers::online,
    ))
}

#[get("/api/v1/workers")]
pub async fn get_workers(
    pool: web::Data<Pool>,
    page: web::Query<Page>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let base = workers_base();

    let records = base
        .paginate(page.into_inner())
        .load::<rebuilderd_common::api::v1::Worker>(connection.as_mut())
        .map_err(Error::from)?;

    let total = base
        .count()
        .get_result::<i64>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(ResultPage { total, records }))
}

#[post("/api/v1/workers")]
pub async fn register_worker(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    request: web::Json<RegisterWorkerRequest>,
) -> web::Result<impl Responder> {
    if auth::worker(&cfg, &req).is_err() {
        return Ok(HttpResponse::Forbidden().finish());
    }

    let mut connection = pool.get().map_err(Error::from)?;
    refresh_worker(&req, &cfg, connection.as_mut())?;

    Ok(HttpResponse::NoContent().finish())
}

#[get("/api/v1/workers/{id}")]
pub async fn get_worker(pool: web::Data<Pool>, id: web::Path<i32>) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    if let Some(record) = workers_base()
        .filter(workers::id.eq(id.into_inner()))
        .get_result::<rebuilderd_common::api::v1::Worker>(connection.as_mut())
        .optional()
        .map_err(Error::from)?
    {
        Ok(HttpResponse::Ok().json(record))
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

#[delete("/api/v1/workers/{id}")]
pub async fn unregister_worker(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    id: web::Path<i32>,
) -> web::Result<impl Responder> {
    if auth::worker(&cfg, &req).is_err() {
        return Ok(HttpResponse::Forbidden().finish());
    }

    let mut connection = pool.get().map_err(Error::from)?;

    connection
        .transaction(|conn| {
            diesel::delete(workers::table)
                .filter(workers::id.eq(id.into_inner()))
                .execute(conn)
        })
        .map_err(Error::from)?;

    Ok(HttpResponse::NoContent().finish())
}
