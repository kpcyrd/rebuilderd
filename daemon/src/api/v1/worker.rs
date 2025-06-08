use crate::api::v1::pagination::Page;
use crate::web;

use crate::db::Pool;
use actix_web::{delete, get, post, HttpResponse, Responder};
use rebuilderd_common::api::v1::RegisterWorkerRequest;
use rebuilderd_common::errors::Error;

#[get("/api/v1/workers")]
pub async fn get_workers(
    pool: web::Data<Pool>,
    page: web::Query<Page>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[post("/api/v1/workers")]
pub async fn register_worker(
    pool: web::Data<Pool>,
    request: web::Json<RegisterWorkerRequest>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/workers/{id}")]
pub async fn get_worker(pool: web::Data<Pool>, id: web::Path<i32>) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[delete("/api/v1/workers/{id}")]
pub async fn unregister_worker(
    pool: web::Data<Pool>,
    id: web::Path<i32>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}
