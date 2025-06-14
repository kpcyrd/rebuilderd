use crate::db::Pool;
use crate::diesel::ExpressionMethods;
use crate::schema::{build_inputs, source_packages};
use crate::web;
use actix_web::{get, HttpResponse, Responder};
use diesel::{QueryDsl, RunQueryDsl};
use rebuilderd_common::errors::Error;

#[get("/api/v1/meta/distributions")]
pub async fn get_distributions(pool: web::Data<Pool>) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let distributions = source_packages::table
        .select(source_packages::distribution)
        .distinct()
        .load::<String>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(distributions))
}

#[get("/api/v1/meta/distributions/{distribution}/releases")]
pub async fn get_distribution_releases(
    pool: web::Data<Pool>,
    distribution: web::Path<String>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let distribution_releases = source_packages::table
        .filter(source_packages::distribution.eq(distribution.into_inner()))
        .select(source_packages::release)
        .distinct()
        .load::<Option<String>>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(distribution_releases))
}

#[get("/api/v1/meta/distributions/{distribution}/architectures")]
pub async fn get_distribution_architectures(
    pool: web::Data<Pool>,
    distribution: web::Path<String>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let distribution_architectures = source_packages::table
        .inner_join(build_inputs::table)
        .filter(source_packages::distribution.eq(distribution.into_inner()))
        .select(build_inputs::architecture)
        .distinct()
        .load::<String>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(distribution_architectures))
}

#[get("/api/v1/meta/distributions/{distribution}/components")]
pub async fn get_distribution_components(
    pool: web::Data<Pool>,
    distribution: web::Path<String>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let distribution_components = source_packages::table
        .filter(source_packages::distribution.eq(distribution.into_inner()))
        .select(source_packages::component)
        .distinct()
        .load::<Option<String>>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(distribution_components))
}

#[get("/api/v1/meta/distributions/{distribution}/{release}/architectures")]
pub async fn get_distribution_release_architectures(
    pool: web::Data<Pool>,
    distribution: web::Path<String>,
    release: web::Path<String>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let distribution_release_architectures = source_packages::table
        .inner_join(build_inputs::table)
        .filter(source_packages::distribution.eq(distribution.into_inner()))
        .filter(source_packages::release.eq(release.into_inner()))
        .select(build_inputs::architecture)
        .distinct()
        .load::<String>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(distribution_release_architectures))
}

#[get("/api/v1/meta/distributions/{distribution}/{release}/components")]
pub async fn get_distribution_release_components(
    pool: web::Data<Pool>,
    distribution: web::Path<String>,
    release: web::Path<String>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let distribution_release_components = source_packages::table
        .filter(source_packages::distribution.eq(distribution.into_inner()))
        .filter(source_packages::release.eq(release.into_inner()))
        .select(source_packages::component)
        .distinct()
        .load::<Option<String>>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(distribution_release_components))
}

#[get("/api/v1/meta/distributions/{distribution}/{release}/components/{component}/architectures")]
pub async fn get_distribution_release_component_architectures(
    pool: web::Data<Pool>,
    distribution: web::Path<String>,
    release: web::Path<String>,
    component: web::Path<String>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let distribution_release_component_architectures = source_packages::table
        .inner_join(build_inputs::table)
        .filter(source_packages::distribution.eq(distribution.into_inner()))
        .filter(source_packages::release.eq(release.into_inner()))
        .filter(source_packages::component.eq(component.into_inner()))
        .select(build_inputs::architecture)
        .distinct()
        .load::<String>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(distribution_release_component_architectures))
}
