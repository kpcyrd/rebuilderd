use crate::api::auth;
use crate::api::v1::util::pagination::{Page, PaginateDsl};
use crate::api::v1::util::worker::refresh_worker;
use crate::config::Config;
use crate::db::Pool;
use crate::diesel::ExpressionMethods;
use crate::schema::workers;
use crate::web;
use actix_web::{delete, get, post, HttpRequest, HttpResponse, Responder};
use diesel::{Connection, OptionalExtension, QueryDsl, RunQueryDsl};
use rebuilderd_common::api::v1::{RegisterWorkerRequest, ResultPage};
use rebuilderd_common::errors::Error;

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

#[get("/")]
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

#[post("/")]
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
