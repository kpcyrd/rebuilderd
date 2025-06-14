use crate::api::v1::util::filters::{IdentityFilter, OriginFilter};
use crate::api::v1::util::pagination::Page;
use crate::db::Pool;
use crate::web;
use actix_web::{get, post, HttpResponse, Responder};
use rebuilderd_common::api::v1::PackageReport;
use rebuilderd_common::errors::Error;

#[post("/api/v1/packages")]
pub async fn submit_package_report(
    pool: web::Data<Pool>,
    request: web::Json<PackageReport>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/packages/source")]
pub async fn get_source_packages(
    pool: web::Data<Pool>,
    page: web::Query<Page>,
    origin_filter: web::Query<OriginFilter>,
    identity_filter: web::Query<IdentityFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/packages/source/{id}")]
pub async fn get_source_package(
    pool: web::Data<Pool>,
    id: web::Path<i32>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/packages/binary")]
pub async fn get_binary_packages(
    pool: web::Data<Pool>,
    page: web::Query<Page>,
    origin_filter: web::Query<OriginFilter>,
    identity_filter: web::Query<IdentityFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/packages/binary/{id}")]
pub async fn get_binary_package(
    pool: web::Data<Pool>,
    id: web::Path<i32>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}
