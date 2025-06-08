use crate::api::header;
use crate::api::v1::util::auth;
use crate::api::v1::util::pagination::PaginateDsl;
use crate::config::Config;
use crate::db::Pool;
use crate::models::NewWorker;
use crate::schema::workers;
use crate::web;
use actix_web::{delete, get, post, HttpRequest, HttpResponse, Responder};
use chrono::Utc;
use diesel::ExpressionMethods;
use diesel::{Connection, OptionalExtension, QueryDsl, RunQueryDsl};
use rebuilderd_common::api::v1::{Page, RegisterWorkerRequest, ResultPage};
use rebuilderd_common::api::WORKER_KEY_HEADER;
use rebuilderd_common::errors::{format_err, Context, Error};
use std::net::IpAddr;

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

#[get("")]
pub async fn get_workers(
    pool: web::Data<Pool>,
    page: web::Query<Page>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let records = workers_base()
        .paginate(page.into_inner())
        .load::<rebuilderd_common::api::v1::Worker>(connection.as_mut())
        .map_err(Error::from)?;

    let total = workers_base()
        .count()
        .get_result::<i64>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(ResultPage { total, records }))
}

#[post("")]
pub async fn register_worker(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    request: web::Json<RegisterWorkerRequest>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;
    if auth::signup(&cfg, &req).is_err() {
        return Ok(HttpResponse::Forbidden().finish());
    }

    let key = header(&req, WORKER_KEY_HEADER).context("Failed to get worker key")?;
    let ip = if let Some(real_ip_header) = &cfg.real_ip_header {
        let ip = header(&req, real_ip_header).context("Failed to locate real ip header")?;
        ip.parse::<IpAddr>()
            .context("Can't parse real ip header as ip address")?
    } else {
        let ci = req
            .peer_addr()
            .ok_or_else(|| format_err!("Can't determine client ip"))?;
        ci.ip()
    };

    let new_worker = NewWorker {
        key: key.to_string(),
        name: request.name.clone(),
        address: ip.to_string(),
        status: None,
        last_ping: Utc::now().naive_utc(),
        online: true,
    };

    new_worker.upsert(connection.as_mut())?;

    Ok(HttpResponse::NoContent().finish())
}

#[get("/{id}")]
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

#[delete("/{id}")]
pub async fn unregister_worker(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    id: web::Path<i32>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;
    if auth::worker(&cfg, &req, connection.as_mut()).is_err() {
        return Ok(HttpResponse::Forbidden().finish());
    }

    connection
        .transaction(|conn| {
            diesel::delete(workers::table)
                .filter(workers::id.eq(id.into_inner()))
                .execute(conn)
        })
        .map_err(Error::from)?;

    Ok(HttpResponse::NoContent().finish())
}
