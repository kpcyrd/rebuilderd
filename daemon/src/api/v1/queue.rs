use crate::api::v1::filters::{IdentityFilter, OriginFilter};
use crate::api::v1::pagination::Page;
use crate::db::Pool;
use crate::web;
use actix_web::{delete, get, post, HttpResponse, Responder};
use rebuilderd_common::api::v1::{PopQueuedJobRequest, QueueJobRequest};
use rebuilderd_common::errors::Error;

#[get("/api/v1/queue")]
pub async fn get_queued_jobs(
    pool: web::Data<Pool>,
    page: web::Query<Page>,
    origin_filter: web::Query<OriginFilter>,
    identity_filter: web::Query<IdentityFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[post("/api/v1/queue")]
pub async fn request_rebuild(
    pool: web::Data<Pool>,
    request: web::Json<QueueJobRequest>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/queue/{id}")]
pub async fn get_queued_job(
    pool: web::Data<Pool>,
    id: web::Path<i32>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[delete("/api/v1/queue/{id}")]
pub async fn drop_queued_job(
    pool: web::Data<Pool>,
    id: web::Path<i32>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[post("/api/v1/queue/pop")]
pub async fn request_work(
    pool: web::Data<Pool>,
    request: web::Json<PopQueuedJobRequest>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}
