use crate::db::Pool;
use crate::web;
use actix_web::{get, HttpResponse, Responder};
use rebuilderd_common::errors::Error;

#[get("/api/v1/meta/distributions")]
pub async fn get_distributions(pool: web::Data<Pool>) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/meta/distributions/{distribution}/releases")]
pub async fn get_distribution_releases(
    pool: web::Data<Pool>,
    distribution: web::Path<String>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/meta/distributions/{distribution}/architectures")]
pub async fn get_distribution_architectures(
    pool: web::Data<Pool>,
    distribution: web::Path<String>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/meta/distributions/{distribution}/components")]
pub async fn get_distribution_components(
    pool: web::Data<Pool>,
    distribution: web::Path<String>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/meta/distributions/{distribution}/{release}/architectures")]
pub async fn get_distribution_release_architectures(
    pool: web::Data<Pool>,
    distribution: web::Path<String>,
    release: web::Path<String>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/meta/distributions/{distribution}/{release}/components")]
pub async fn get_distribution_release_components(
    pool: web::Data<Pool>,
    distribution: web::Path<String>,
    release: web::Path<String>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/meta/distributions/{distribution}/{release}/components/{component}/architectures")]
pub async fn get_distribution_release_component_architectures(
    pool: web::Data<Pool>,
    distribution: web::Path<String>,
    release: web::Path<String>,
    component: web::Path<String>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}
