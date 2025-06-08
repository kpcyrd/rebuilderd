use crate::web;
use actix_web::{get, HttpResponse, Responder};

#[get("/api/v1/meta/distributions")]
pub async fn get_distributions() -> web::Result<impl Responder> {
    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/meta/distributions/{distribution}/releases")]
pub async fn get_distribution_releases(
    distribution: web::Path<String>,
) -> web::Result<impl Responder> {
    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/meta/distributions/{distribution}/architectures")]
pub async fn get_distribution_architectures(
    distribution: web::Path<String>,
) -> web::Result<impl Responder> {
    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/meta/distributions/{distribution}/components")]
pub async fn get_distribution_components(
    distribution: web::Path<String>,
) -> web::Result<impl Responder> {
    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/meta/distributions/{distribution}/{release}/architectures")]
pub async fn get_distribution_release_architectures(
    distribution: web::Path<String>,
    release: web::Path<String>,
) -> web::Result<impl Responder> {
    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/meta/distributions/{distribution}/{release}/components")]
pub async fn get_distribution_release_components(
    distribution: web::Path<String>,
    release: web::Path<String>,
) -> web::Result<impl Responder> {
    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/meta/distributions/{distribution}/{release}/components/{component}/architectures")]
pub async fn get_distribution_release_component_architectures(
    distribution: web::Path<String>,
    release: web::Path<String>,
    component: web::Path<String>,
) -> web::Result<impl Responder> {
    Ok(HttpResponse::NotImplemented())
}
